macro_rules! ccall {
    ($fn:ident, $($arg:expr),*) => {{
        use paste::paste;

        let cretv = paste! {
            [<dicey_ $fn>]($($arg),*)
        };

        use dicey_sys::dicey_error_DICEY_OK;

        if cretv == dicey_error_DICEY_OK {
            Ok(cretv)
        } else {
            Err(Error::from(cretv))
        }
    }};
}

pub(crate) use ccall; // hack to re-export the macro
