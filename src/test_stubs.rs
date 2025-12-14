// Licensed under either of:
/*
 * Apache License, Version 2.0:
 *
 * Copyright 2025 Sašo Kiselkov
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */
/*
 * MIT license:
 *
 * Copyright (c) 2025 Sašo Kiselkov
 *
 * Permission is hereby granted, free of charge, to any pony obtaining a
 * copy of this software and associated documentation files (the "Software"),
 * to deal in the Software without restriction, including without limitation
 * the rights to use, copy, modify, merge, publish, distribute, sublicense,
 * and/or sell copies of the Software, and to permit ponies to whom the
 * Software is furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL
 * THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
 * DEALINGS IN THE SOFTWARE.
 */

use core::slice;
use std::{
    cell::OnceCell,
    collections::HashMap,
    ffi::{c_char, c_double, c_float, c_int, c_void, CStr},
    sync::Mutex,
};

use xplm_sys::{XPLMDataRef, XPLMDataTypeID};

static TEST_STUB_DATAREFS: Mutex<OnceCell<HashMap<&'static CStr, TestDataRef>>> =
    Mutex::new(OnceCell::new());

const TEST_ARRAY_LEN: usize = 5;

#[allow(dead_code)]
#[derive(Debug)]
enum TestDataRef {
    I32(i32),
    F32(f32),
    F64(f64),
    I32Array([i32; TEST_ARRAY_LEN]),
    F32Array([f32; TEST_ARRAY_LEN]),
    ByteArray([u8; TEST_ARRAY_LEN]),
}

fn create_test_drs() -> HashMap<&'static CStr, TestDataRef> {
    let mut map = HashMap::new();
    map.insert(c"test/i32", TestDataRef::I32(0));
    map.insert(c"test/f32", TestDataRef::F32(0.0));
    map.insert(c"test/f64", TestDataRef::F64(0.0));
    map.insert(c"test/i32array", TestDataRef::I32Array([0; TEST_ARRAY_LEN]));
    map.insert(
        c"test/f32array",
        TestDataRef::F32Array([0.0; TEST_ARRAY_LEN]),
    );
    map.insert(
        c"test/bytearray",
        TestDataRef::ByteArray([0; TEST_ARRAY_LEN]),
    );
    map
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetDataRefTypes(dr: XPLMDataRef) -> XPLMDataTypeID {
    let dr = dr as *const TestDataRef;
    let dr = unsafe { dr.as_ref().unwrap() };
    match dr {
        TestDataRef::I32(_) => xplm_sys::xplmType_Int as _,
        TestDataRef::F32(_) => xplm_sys::xplmType_Float as _,
        TestDataRef::F64(_) => xplm_sys::xplmType_Double as _,
        TestDataRef::I32Array(_) => xplm_sys::xplmType_IntArray as _,
        TestDataRef::F32Array(_) => xplm_sys::xplmType_FloatArray as _,
        TestDataRef::ByteArray(_) => xplm_sys::xplmType_Data as _,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMFindDataRef(name: *const c_char) -> XPLMDataRef {
    let name = unsafe { CStr::from_ptr(name) };
    let datarefs = TEST_STUB_DATAREFS.lock().unwrap();
    let datarefs = datarefs.get_or_init(create_test_drs);
    let Some(dr) = datarefs.get(name) else {
        return std::ptr::null_mut();
    };
    let dr_ptr: *const TestDataRef = dr;
    dr_ptr as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMCanWriteDataRef(_: XPLMDataRef) -> c_int {
    1
}

macro_rules! impl_scalar_dr_accessors {
    ($getter:ident, $setter:ident, $c_type:ty, $variant:ident) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $getter(dr: XPLMDataRef) -> $c_type {
            let dr = unsafe { (dr as *const TestDataRef).as_ref().unwrap() };
            match dr {
                TestDataRef::$variant(inner_value) => *inner_value,
                _ => panic!("attempted to {} from dataref {dr:?}", stringify!($getter)),
            }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $setter(dr: XPLMDataRef, value: $c_type) {
            let dr = unsafe { (dr as *mut TestDataRef).as_mut().unwrap() };
            match dr {
                TestDataRef::$variant(inner_value) => *inner_value = value,
                _ => panic!("attempted to write {value:?} into dataref {dr:?}"),
            }
        }
    };
}

impl_scalar_dr_accessors!(XPLMGetDatai, XPLMSetDatai, c_int, I32);
impl_scalar_dr_accessors!(XPLMGetDataf, XPLMSetDataf, c_float, F32);
impl_scalar_dr_accessors!(XPLMGetDatad, XPLMSetDatad, c_double, F64);

macro_rules! impl_vector_dr_accessors {
    ($getter:ident, $setter:ident, $c_type:ty, $variant:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $getter(
            dr: XPLMDataRef,
            dest: *mut $c_type,
            start_offset: c_int,
            out_cap: c_int,
        ) -> c_int {
            let dr = unsafe { (dr as *const TestDataRef).as_ref().unwrap() };
            let start_offset = usize::try_from(start_offset).unwrap();
            match dr {
                TestDataRef::$variant(src) => {
                    let dest = unsafe { slice::from_raw_parts_mut(dest as *mut _, out_cap as _) };
                    let copy_length = dest.len().min(src.len().saturating_sub(start_offset));
                    let end_offset = start_offset + copy_length;
                    dest[..copy_length].copy_from_slice(&src[start_offset..end_offset]);
                    copy_length as _
                }
                _ => panic!("attempted to {} from dataref {dr:?}", stringify!($getter)),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $setter(
            dr: XPLMDataRef,
            src: *mut $c_type,
            start_offset: c_int,
            out_cap: c_int,
        ) -> c_int {
            let dr = unsafe { (dr as *mut TestDataRef).as_mut().unwrap() };
            let start_offset = usize::try_from(start_offset).unwrap();
            match dr {
                TestDataRef::$variant(dest) => {
                    let src = unsafe { slice::from_raw_parts(src as *const _, out_cap as _) };
                    let copy_length = src.len().min(dest.len().saturating_sub(start_offset));
                    let end_offset = start_offset + copy_length;
                    dest[start_offset..end_offset].copy_from_slice(&src[..copy_length]);
                    copy_length as _
                }
                _ => panic!("attempted to {} into dataref {dr:?}", stringify!($setter)),
            }
        }
    };
}

impl_vector_dr_accessors!(XPLMGetDatavi, XPLMSetDatavi, c_int, I32Array);
impl_vector_dr_accessors!(XPLMGetDatavf, XPLMSetDatavf, c_float, F32Array);
impl_vector_dr_accessors!(XPLMGetDatab, XPLMSetDatab, c_void, ByteArray);
