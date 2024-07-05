use std::{
    ffi::c_char,
    mem::{self, ManuallyDrop},
    os::raw::c_void,
    pin::Pin,
    ptr,
    sync::Arc,
};

use crate::{
    core::{macros::ccall, value::Path},
    Error, FromDicey, Message, MessageBuilder, ObjectInfo, Op, Selector, ToDicey, ValueBuilder,
    ValueView,
};

use super::{address::Address, DEFAULT_TIMEOUT_MS};

use dicey_sys::{
    dicey_client, dicey_client_args, dicey_client_connect_async, dicey_client_delete,
    dicey_client_disconnect, dicey_client_get_context, dicey_client_is_running, dicey_client_new,
    dicey_client_request_async, dicey_client_set_context, dicey_error, dicey_packet,
    dicey_packet_is_valid, DICEY_EVENTMANAGER_SUBSCRIBE_OP_NAME, DICEY_EVENTMANAGER_TRAIT_NAME,
    DICEY_EVENTMANAGER_UNSUBSCRIBE_OP_NAME, DICEY_INTROSPECTION_DATA_PROP_NAME,
    DICEY_INTROSPECTION_TRAIT_NAME, DICEY_INTROSPECTION_XML_PROP_NAME, DICEY_SERVER_PATH,
};

use futures::channel::oneshot;
use tokio::sync::broadcast::{Receiver, Sender};

pub const DEFAULT_EVENT_QUEUE_SIZE: usize = 32usize;

pub struct ClientArgs<A: Into<Address>> {
    pub pipe: A,
    pub event_queue_size: usize,
}

pub struct Client {
    state: Pin<Box<ClientState>>,
}

impl Client {
    pub async fn connect(pipe: impl Into<Address>) -> Result<Self, Error> {
        Self::connect_with_args(ClientArgs {
            pipe,
            event_queue_size: DEFAULT_EVENT_QUEUE_SIZE,
        })
        .await
    }

    pub async fn connect_with_args<A: Into<Address>>(
        ClientArgs {
            pipe,
            event_queue_size,
        }: ClientArgs<A>,
    ) -> Result<Self, Error> {
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
                events: Sender::new(event_queue_size),
            }),
        };

        type ConnectResult = Result<(), Error>;
        type Tx = oneshot::Sender<ConnectResult>;

        extern "C" fn connect_cb(
            client: *mut dicey_client,
            ctx: *mut c_void,
            status: dicey_error,
            _: *const c_char,
        ) {
            debug_assert!(!client.is_null() && !ctx.is_null());

            let tx = unsafe { ManuallyDrop::into_inner(ptr::read(ctx as *mut ManuallyDrop<Tx>)) };

            let status = Error::from(status);

            tx.send(if status == Error::OK {
                Ok(())
            } else {
                Err(status)
            })
            .expect("receiver should never drop before here")
        }

        let (tx, rx) = oneshot::channel::<ConnectResult>();

        let mut tx = ManuallyDrop::new(tx);

        unsafe {
            dicey_client_set_context(ptr, &mut *client.state as *mut _ as *mut c_void);

            if let Err(err) = ccall!(
                client_connect_async,
                client.ptr(),
                addr.into_raw(),
                Some(connect_cb),
                &mut tx as *mut _ as *mut c_void
            ) {
                ManuallyDrop::drop(&mut tx);

                return Err(err);
            }
        }

        rx.await
            .expect("sender should never drop before here")
            .map(|_| client)
    }

    pub fn events(&self) -> EventSource {
        EventSource {
            events: self.state.events.subscribe(),
        }
    }

    pub async fn exec<'b>(
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
            .await
    }

    pub async fn get<'b>(
        &self,
        path: impl Into<Vec<u8>>,
        selector: impl Into<Selector<'b>>,
    ) -> Result<Message, Error> {
        self.request(Op::Get)
            .path(path)?
            .selector(selector)?
            .submit()
            .await
    }

    pub async fn inspect(&self, path: impl Into<Vec<u8>>) -> Result<ObjectInfo, Error> {
        let path = path.into();
        let path_str = String::from_utf8(path.clone()).map_err(|_| Error::InvalidData)?;

        self.get(
            path,
            (
                DICEY_INTROSPECTION_TRAIT_NAME.as_ref(),
                DICEY_INTROSPECTION_DATA_PROP_NAME.as_ref(),
            ),
        )
        .await
        .and_then(move |m| match m.value() {
            Some(ValueView::Error(e)) => Err(Error::from(e.code as dicey_error)),
            Some(ref view) => ObjectInfo::from_dicey(path_str, view),
            _ => Err(Error::BadMessage),
        })
    }

    pub async fn inspect_as_xml(&self, path: impl Into<Vec<u8>>) -> Result<String, Error> {
        self.get(
            path,
            (
                DICEY_INTROSPECTION_TRAIT_NAME.as_ref(),
                DICEY_INTROSPECTION_XML_PROP_NAME.as_ref(),
            ),
        )
        .await
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

    pub async fn request_with(&self, msg: Message, timeout_ms: u32) -> Result<Message, Error> {
        type RespResult = Result<Message, Error>;
        type Tx = oneshot::Sender<RespResult>;

        extern "C" fn reply_cb(
            client: *mut dicey_client,
            ctx: *mut c_void,
            status: dicey_error,
            packet: *mut dicey_packet,
        ) {
            debug_assert!(!client.is_null() && !ctx.is_null() && !packet.is_null());

            let tx = unsafe { ManuallyDrop::into_inner(ptr::read(ctx as *mut ManuallyDrop<Tx>)) };

            let status = Error::from(status);

            tx.send(if status == Error::OK {
                let packet = unsafe { ptr::replace(packet, mem::zeroed()) };

                Message::from_raw(packet)
            } else {
                Err(status)
            })
            .expect("receiver should never drop before here")
        }

        let (tx, rx) = oneshot::channel::<RespResult>();

        let mut tx = ManuallyDrop::new(tx);

        unsafe {
            if let Err(err) = ccall!(
                client_request_async,
                self.ptr(),
                msg.into_raw(),
                Some(reply_cb),
                &mut tx as *mut _ as *mut c_void,
                timeout_ms
            ) {
                ManuallyDrop::drop(&mut tx);

                return Err(err);
            }
        }

        rx.await.expect("sender should never drop before here")
    }

    pub async fn set<'b>(
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
            .submit()
            .await?;

        debug_assert!(msg
            .value()
            .and_then(|ref v| <()>::from_dicey(v).ok())
            .is_some());

        Ok(())
    }

    pub async fn subscribe_to<'b>(
        &self,
        path: impl AsRef<str>,
        selector: impl Into<Selector<'b>>,
    ) -> Result<(), Error> {
        // strip the null terminator, otherwise path() and friends will choke due to `CString::new` actively checking
        // null characters
        let server_path = &DICEY_SERVER_PATH[..DICEY_SERVER_PATH.len() - 1];
        let eventmanager_trait_name =
            &DICEY_EVENTMANAGER_TRAIT_NAME[..DICEY_EVENTMANAGER_TRAIT_NAME.len() - 1];
        let eventmanager_subscribe_op_name =
            &DICEY_EVENTMANAGER_SUBSCRIBE_OP_NAME[..DICEY_EVENTMANAGER_SUBSCRIBE_OP_NAME.len() - 1];

        self.request(Op::Exec)
            .path(server_path)?
            .selector(Selector {
                trait_name: eventmanager_trait_name,
                elem: eventmanager_subscribe_op_name,
            })?
            .value((Path(path.as_ref()), selector.into()))?
            .submit()
            .await
            .and_then(|m| match m.value() {
                Some(ValueView::Unit) => Ok(()),
                Some(ValueView::Error(e)) => Err(Error::from(e.code as dicey_error)),
                _ => Err(Error::BadMessage),
            })
    }

    pub async fn unsubscribe_from<'b>(
        &self,
        path: impl AsRef<str>,
        selector: impl Into<Selector<'b>>,
    ) -> Result<(), Error> {
        self.request(Op::Exec)
            .path(DICEY_SERVER_PATH)?
            .selector(Selector {
                trait_name: DICEY_EVENTMANAGER_TRAIT_NAME,
                elem: DICEY_EVENTMANAGER_UNSUBSCRIBE_OP_NAME,
            })?
            .value((Path(path.as_ref()), selector.into()))?
            .submit()
            .await
            .and_then(|m| match m.value() {
                Some(ValueView::Unit) => Ok(()),
                Some(ValueView::Error(e)) => Err(Error::from(e.code as dicey_error)),
                _ => Err(Error::BadMessage),
            })
    }

    fn ptr(&self) -> *mut dicey_client {
        self.state.ptr
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            //attempt disconnecting. We don't really care about the result.
            dicey_client_disconnect(self.ptr());

            dicey_client_delete(self.ptr());
        }
    }
}

pub struct EventSource {
    events: Receiver<Arc<Message>>,
}

impl EventSource {
    pub async fn next(&mut self) -> Result<Arc<Message>, Error> {
        self.events.recv().await.map_err(|e| {
            use tokio::sync::broadcast::error::RecvError::*;

            match e {
                Closed => Error::Cancelled,
                Lagged(_) => Error::TimedOut,
            }
        })
    }

    pub fn poll(&mut self) -> Result<Arc<Message>, Error> {
        use tokio::sync::broadcast::error::TryRecvError::*;

        match self.events.try_recv() {
            Ok(msg) => Ok(msg),
            Err(Empty) => Err(Error::TryAgain),
            Err(Closed) => Err(Error::Cancelled),
            Err(Lagged(_)) => Err(Error::TimedOut),
        }
    }
}

pub struct RequestBuilder<'a> {
    client: &'a Client,

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

    pub async fn submit(self) -> Result<Message, Error> {
        self.client
            .request_with(self.mbuilder.build()?, self.timeout_ms)
            .await
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
struct ClientState {
    ptr: *mut dicey_client,

    events: Sender<Arc<Message>>,
}

unsafe extern "C" fn client_on_event(
    c_client: *mut dicey_client,
    ctx: *mut ::std::os::raw::c_void,
    packet: *mut dicey_packet,
) {
    let state = {
        assert!(!c_client.is_null() && !ctx.is_null() && dicey_packet_is_valid(*packet));

        &mut *(dicey_client_get_context(c_client) as *mut ClientState)
    };

    // if there are no subscribers, we can just drop the message
    let _ = state.events.send(Arc::new(
        Message::from_raw(ptr::replace(packet, mem::zeroed()))
            .expect("failed to convert packet to message"),
    ));
}
