use std::collections::HashMap;

use dicey_sys::{
    dicey_element_type_DICEY_ELEMENT_TYPE_OPERATION,
    dicey_element_type_DICEY_ELEMENT_TYPE_PROPERTY, dicey_element_type_DICEY_ELEMENT_TYPE_SIGNAL,
};

use crate::{Error, FromDicey, ValueView};

#[derive(Clone, Debug)]
pub struct ObjectInfo {
    pub path: String,
    pub traits: Traits,
}

impl ObjectInfo {
    pub(crate) fn new(path: String, traits: Traits) -> Self {
        Self { path, traits }
    }

    pub(crate) fn from_dicey(
        path: String,
        value: &crate::ValueView<'_>,
    ) -> Result<Self, crate::Error> {
        Ok(Self::new(path, Traits::from_dicey(value)?))
    }
}

#[derive(Clone, Debug)]
pub enum Element {
    Operation(Operation),
    Property(Property),
    Signal(Signal),
}

#[derive(Clone, Debug)]
pub struct Elements(HashMap<String, Element>);

impl Elements {
    pub fn elements(&self) -> impl Iterator<Item = (&String, &Element)> {
        self.0.iter()
    }

    pub fn operations(&self) -> impl Iterator<Item = (&String, &Operation)> {
        self.elements().filter_map(|(name, element)| {
            if let Element::Operation(operation) = element {
                Some((name, operation))
            } else {
                None
            }
        })
    }

    pub fn properties(&self) -> impl Iterator<Item = (&String, &Property)> {
        self.elements().filter_map(|(name, element)| {
            if let Element::Property(property) = element {
                Some((name, property))
            } else {
                None
            }
        })
    }

    pub fn signals(&self) -> impl Iterator<Item = (&String, &Signal)> {
        self.elements().filter_map(|(name, element)| {
            if let Element::Signal(signal) = element {
                Some((name, signal))
            } else {
                None
            }
        })
    }
}

#[derive(Clone, Debug)]
pub struct Operation {
    pub signature: String,
}

#[derive(Clone, Debug)]
pub struct Property {
    pub signature: String,
    pub readonly: bool,
}

#[derive(Clone, Debug)]
pub struct Signal {
    pub signature: String,
}

pub type Traits = HashMap<String, Elements>;

impl<'a> FromDicey<'a> for Traits {
    fn from_dicey(value: &ValueView<'a>) -> Result<Self, Error> {
        // let list = if let ValueView::Array { items, .. } = value {
        //     items
        // } else {
        //     return Err(crate::Error::ValueTypeMismatch);
        // };

        // let mut traits = Traits::with_capacity(list.len());

        // for trait_def in list {

        // }

        value.extract::<Vec<(&str, Vec<(&str, ValueView<'a>)>)>>()?.into_iter().map(|(tname, entries)| {
            let entries = entries.into_iter().map(|(ename, value)| {
                let entries = if let ValueView::Tuple(entries) = value {
                    entries
                } else {
                    return Err(crate::Error::ValueTypeMismatch);
                };

                let (kind, sig, readonly) = match &entries[..] {
                    [ValueView::Byte(kind), ValueView::String(sig), ValueView::Bool(ro)] => (kind, sig, *ro),
                    [ValueView::Byte(kind), ValueView::String(sig)] => (kind, sig, false),
                    _ => return Err(crate::Error::ValueTypeMismatch),
                };

                let element = match kind.0 as u32 {
                    dicey_element_type_DICEY_ELEMENT_TYPE_OPERATION => Element::Operation(Operation { signature: sig.to_string() }),
                    dicey_element_type_DICEY_ELEMENT_TYPE_PROPERTY => Element::Property(Property {
                        signature: sig.to_string(),
                        readonly
                    }),
                    dicey_element_type_DICEY_ELEMENT_TYPE_SIGNAL => Element::Signal(Signal { signature: sig.to_string() }),
                    _ => return Err(Error::BadMessage),
                };

                Ok((ename.to_string(), element))
            }).collect::<Result<_, _>>()?;

            Ok((tname.to_string(), Elements(entries)))
        }).collect()
    }
}
