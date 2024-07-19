/*
 * Copyright (c) 2014-2024 Zuru Tech HK Limited, All rights reserved.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

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
