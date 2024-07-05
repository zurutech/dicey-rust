use std::{
    ffi::{c_char, CString},
    mem,
    os::raw::c_void,
    pin::Pin,
    ptr,
};

use dicey_sys::{
    dicey_client, dicey_client_args, dicey_client_connect, dicey_client_delete,
    dicey_client_disconnect, dicey_client_get_context, dicey_client_is_running, dicey_client_new,
    dicey_client_request, dicey_client_set_context, dicey_client_subscribe_to,
    dicey_client_unsubscribe_from, dicey_error, dicey_packet, dicey_packet_is_valid,
    dicey_selector, DICEY_INTROSPECTION_DATA_PROP_NAME, DICEY_INTROSPECTION_TRAIT_NAME,
    DICEY_INTROSPECTION_XML_PROP_NAME,
};

use crate::{
    core::{
        macros::ccall,
        value::{bytes_to_cpath, FromDicey},
    },
    Error, Message, MessageBuilder, ObjectInfo, Op, Selector, ToDicey, ValueBuilder, ValueView,
};

use super::{address::Address, DEFAULT_TIMEOUT_MS};

pub trait EventHandler: FnMut(Message) + Send + Sync {}

impl<F: FnMut(Message) + Send + Sync> EventHandler for F {}

pub struct ClientArgs<A: Into<Address>, F: EventHandler> {
    pub pipe: A,
    pub on_event: Option<F>,
}

pub struct Client<'a> {
    state: Pin<Box<ClientState<'a>>>,
}

impl<'a> Client<'a> {
    pub fn connect<A, F>(ClientArgs { pipe, on_event }: ClientArgs<A, F>) -> Result<Self, Error>
    where
        A: Into<Address>,
        F: EventHandler + 'a,
    {
        let addr = pipe.into();

        let ptr = unsafe {
            let mut cln = ptr::null_mut();

            ccall!(
                client_new,
                &mut cln,
                &dicey_client_args {
                    inspect_func: None,
                    on_event: Some(client_on_event),
                }
            )?;

            cln
        };

        // ensure that Drop will run if something goes wrong
        let mut client = Self {
            state: Box::pin(ClientState {
                ptr,
                on_event: on_event.map(|f| Box::new(f) as Box<dyn FnMut(Message)>),
            }),
        };

        unsafe {
            dicey_client_set_context(ptr, &mut *client.state as *mut _ as *mut c_void);

            ccall!(client_connect, client.ptr(), addr.into_raw())?;
        }

        Ok(client)
    }

    pub fn exec<'b>(
        &self,
        path: impl Into<Vec<u8>>,
        selector: impl Into<Selector<'b>>,
        argument: impl ToDicey,
    ) -> Result<Message, Error> {
        self.request(Op::Exec)
            .path(path)?
            .selector(selector)?
            .value(argument)?
            .submit()
    }

    pub fn get<'b>(
        &self,
        path: impl Into<Vec<u8>>,
        selector: impl Into<Selector<'b>>,
    ) -> Result<Message, Error> {
        self.request(Op::Get)
            .path(path)?
            .selector(selector)?
            .submit()
    }

    pub fn inspect(&self, path: impl Into<Vec<u8>>) -> Result<ObjectInfo, Error> {
        let path = path.into();
        let path_str = String::from_utf8(path.clone()).map_err(|_| Error::InvalidData)?;

        self.get(
            path,
            (
                DICEY_INTROSPECTION_TRAIT_NAME.as_ref(),
                DICEY_INTROSPECTION_DATA_PROP_NAME.as_ref(),
            ),
        )
        .and_then(move |m| match m.value() {
            Some(ValueView::Error(e)) => Err(Error::from(e.code as dicey_error)),
            Some(ref view) => ObjectInfo::from_dicey(path_str, view),
            _ => Err(Error::BadMessage),
        })
    }

    pub fn inspect_as_xml(&self, path: impl Into<Vec<u8>>) -> Result<String, Error> {
        self.get(
            path,
            (
                DICEY_INTROSPECTION_TRAIT_NAME.as_ref(),
                DICEY_INTROSPECTION_XML_PROP_NAME.as_ref(),
            ),
        )
        .and_then(|m| match m.value() {
            Some(ValueView::String(s)) => Ok(s.to_owned()),
            Some(ValueView::Error(e)) => Err(Error::from(e.code as dicey_error)),
            _ => Err(Error::BadMessage),
        })
    }

    pub fn is_running(&self) -> bool {
        !self.ptr().is_null() && unsafe { dicey_client_is_running(self.ptr()) }
    }

    pub fn request(&self, op: Op) -> RequestBuilder {
        RequestBuilder::new(self, op)
    }

    pub fn request_with(&self, msg: Message, timeout_ms: u32) -> Result<Message, Error> {
        unsafe {
            let mut c_resp = mem::zeroed();

            ccall!(
                client_request,
                self.ptr(),
                msg.into_raw(),
                &mut c_resp,
                timeout_ms
            )?;

            Message::from_raw(c_resp)
        }
    }

    pub fn set<'b>(
        &self,
        path: impl Into<Vec<u8>>,
        selector: impl Into<Selector<'b>>,
        argument: impl ToDicey,
    ) -> Result<(), Error> {
        let msg = self
            .request(Op::Set)
            .path(path)?
            .selector(selector)?
            .value(argument)?
            .submit()?;

        debug_assert!(msg
            .value()
            .and_then(|ref v| <()>::from_dicey(v).ok())
            .is_some());

        Ok(())
    }

    pub fn subscribe_to<'b>(
        &self,
        path: impl Into<Vec<u8>>,
        selector: impl Into<Selector<'b>>,
    ) -> Result<(), Error> {
        let cpath = bytes_to_cpath(path)?;

        let sel = selector.into();

        let tname = Some(CString::new(sel.trait_name).map_err(|_| Error::InvalidData)?);
        let elem = Some(CString::new(sel.elem).map_err(|_| Error::InvalidData)?);

        let csel = dicey_selector {
            trait_: tname.as_ref().unwrap().as_ptr() as *const c_char,
            elem: elem.as_ref().unwrap().as_ptr() as *const c_char,
        };

        unsafe {
            ccall!(
                client_subscribe_to,
                self.ptr(),
                cpath.as_ptr(),
                csel,
                DEFAULT_TIMEOUT_MS
            )
        }
        .map(|_| ())
    }

    pub fn unsubscribe_from<'b>(
        &self,
        path: impl Into<Vec<u8>>,
        selector: impl Into<Selector<'b>>,
    ) -> Result<(), Error> {
        let cpath = bytes_to_cpath(path)?;

        let sel = selector.into();

        let tname = Some(CString::new(sel.trait_name).map_err(|_| Error::InvalidData)?);
        let elem = Some(CString::new(sel.elem).map_err(|_| Error::InvalidData)?);

        let csel = dicey_selector {
            trait_: tname.as_ref().unwrap().as_ptr() as *const c_char,
            elem: elem.as_ref().unwrap().as_ptr() as *const c_char,
        };

        unsafe {
            ccall!(
                client_unsubscribe_from,
                self.ptr(),
                cpath.as_ptr(),
                csel,
                DEFAULT_TIMEOUT_MS
            )
        }
        .map(|_| ())
    }

    fn ptr(&self) -> *mut dicey_client {
        self.state.ptr
    }
}

impl Drop for Client<'_> {
    fn drop(&mut self) {
        unsafe {
            //attempt disconnecting. We don't really care about the result.
            dicey_client_disconnect(self.ptr());

            dicey_client_delete(self.ptr());
        }
    }
}

pub struct RequestBuilder<'a> {
    client: &'a Client<'a>,

    mbuilder: MessageBuilder,
    timeout_ms: u32,
}

impl<'a> RequestBuilder<'a> {
    fn new(client: &'a Client, op: Op) -> Self {
        Self {
            client,
            mbuilder: MessageBuilder::new(op)
                .expect("failed to create message builder (out of memory?)"),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    pub fn path(self, path: impl Into<Vec<u8>>) -> Result<Self, Error> {
        Ok(Self {
            mbuilder: self.mbuilder.path(path)?,
            ..self
        })
    }

    pub fn selector<'b>(self, sel: impl Into<Selector<'b>>) -> Result<Self, Error> {
        Ok(Self {
            mbuilder: self.mbuilder.selector(sel)?,
            ..self
        })
    }

    pub fn seq(self, seq: u32) -> Self {
        Self {
            mbuilder: self.mbuilder.seq(seq),
            ..self
        }
    }

    pub fn submit(self) -> Result<Message, Error> {
        self.client
            .request_with(self.mbuilder.build()?, self.timeout_ms)
    }

    pub fn timeout(self, timeout_ms: u32) -> Self {
        Self { timeout_ms, ..self }
    }

    pub fn value(self, value: impl ToDicey) -> Result<Self, Error> {
        Ok(Self {
            mbuilder: self.mbuilder.value(value)?,
            ..self
        })
    }

    pub fn value_with<F>(self, op: F) -> Result<Self, Error>
    where
        F: FnOnce(&mut ValueBuilder) -> Result<(), Error>,
    {
        Ok(Self {
            mbuilder: self.mbuilder.value_with(op)?,
            ..self
        })
    }
}

// we must put the client internal state in a separate struct we then allocate into the heap,
// otherwise we can't really pin it to a specific memory location
struct ClientState<'a> {
    ptr: *mut dicey_client,

    on_event: Option<Box<dyn FnMut(Message) + 'a>>,
}

unsafe extern "C" fn client_on_event(
    c_client: *mut dicey_client,
    ctx: *mut ::std::os::raw::c_void,
    packet: *mut dicey_packet,
) {
    let state = unsafe {
        assert!(!c_client.is_null() && !ctx.is_null() && dicey_packet_is_valid(*packet));

        &mut *(dicey_client_get_context(c_client) as *mut ClientState)
    };

    if let Some(cb) = state.on_event.as_mut() {
        let message = Message::from_raw(ptr::replace(packet, mem::zeroed()))
            .expect("failed to convert packet to message");

        cb(message);
    }
}
