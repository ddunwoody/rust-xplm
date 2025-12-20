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

#![allow(clippy::missing_safety_doc)]

use core::slice;
use std::{
    collections::HashMap,
    ffi::{c_char, c_double, c_float, c_int, c_void, CStr, CString},
    pin::Pin,
    sync::Mutex,
};

use xplm_sys::{
    xplmType_Data, xplmType_Double, xplmType_Float, xplmType_FloatArray, xplmType_Int,
    xplmType_IntArray, XPLMDataRef, XPLMDataTypeID, XPLMGetDatab_f, XPLMGetDatad_f, XPLMGetDataf_f,
    XPLMGetDatai_f, XPLMGetDatavf_f, XPLMGetDatavi_f, XPLMPluginID, XPLMSetDatab_f, XPLMSetDatad_f,
    XPLMSetDataf_f, XPLMSetDatai_f, XPLMSetDatavf_f, XPLMSetDatavi_f, XPLM_NO_PLUGIN_ID,
};

pub static DATAREF_SYS_LOCK: Mutex<()> = Mutex::new(());

static STUB_DATAREFS: Mutex<Option<HashMap<CString, Pin<Box<TestData>>>>> = Mutex::new(None);

const TEST_ARRAY_LEN: usize = 5;

#[allow(dead_code)]
#[derive(Debug)]
struct TestData {
    name: CString,
    data: TestDataPayload,
}

#[derive(Debug)]
enum TestDataPayload {
    OwnedI32(i32),
    OwnedF32(f32),
    OwnedF64(f64),
    OwnedI32Array([i32; TEST_ARRAY_LEN]),
    OwnedF32Array([f32; TEST_ARRAY_LEN]),
    OwnedByteArray([u8; TEST_ARRAY_LEN]),
    Refd(TestDataRef),
}

#[derive(Debug)]
struct TestDataRef {
    typ: XPLMDataTypeID,
    writable: bool,
    read_i32: XPLMGetDatai_f,
    write_i32: XPLMSetDatai_f,
    read_f32: XPLMGetDataf_f,
    write_f32: XPLMSetDataf_f,
    read_f64: XPLMGetDatad_f,
    write_f64: XPLMSetDatad_f,
    read_i32_array: XPLMGetDatavi_f,
    write_i32_array: XPLMSetDatavi_f,
    read_f32_array: XPLMGetDatavf_f,
    write_f32_array: XPLMSetDatavf_f,
    read_data: XPLMGetDatab_f,
    write_data: XPLMSetDatab_f,
    read_refcon: *mut c_void,
    write_refcon: *mut c_void,
}
unsafe impl Send for TestDataRef {}

fn create_test_drs() -> HashMap<CString, Pin<Box<TestData>>> {
    macro_rules! make_dataref {
        ($map:expr, $name:literal, $variant:ident, $payload:expr$(,)?) => {
            $map.insert(
                $name.into(),
                Box::pin(TestData {
                    name: $name.into(),
                    data: TestDataPayload::$variant($payload),
                }),
            );
        };
    }

    let mut map = HashMap::new();
    make_dataref!(map, c"test/i32", OwnedI32, 0);
    make_dataref!(map, c"test/f32", OwnedF32, 0.0);
    make_dataref!(map, c"test/f64", OwnedF64, 0.0);
    make_dataref!(map, c"test/i32array", OwnedI32Array, [0; TEST_ARRAY_LEN]);
    make_dataref!(map, c"test/f32array", OwnedF32Array, [0.0; TEST_ARRAY_LEN]);
    make_dataref!(map, c"test/bytearray", OwnedByteArray, [0; TEST_ARRAY_LEN]);
    make_dataref!(map, c"sim/aircraft/autopilot/dg_source", OwnedI32, 10);
    make_dataref!(
        map,
        c"sim/flightmodel/engine/ENGN_propmode",
        OwnedI32Array,
        [0; TEST_ARRAY_LEN],
    );
    map
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetDataRefTypes(dr: XPLMDataRef) -> XPLMDataTypeID {
    let dr = dr as *const TestData;
    let dr = unsafe { dr.as_ref().unwrap() };
    match &dr.data {
        TestDataPayload::OwnedI32(_) => xplm_sys::xplmType_Int as _,
        TestDataPayload::OwnedF32(_) => xplm_sys::xplmType_Float as _,
        TestDataPayload::OwnedF64(_) => xplm_sys::xplmType_Double as _,
        TestDataPayload::OwnedI32Array(_) => xplm_sys::xplmType_IntArray as _,
        TestDataPayload::OwnedF32Array(_) => xplm_sys::xplmType_FloatArray as _,
        TestDataPayload::OwnedByteArray(_) => xplm_sys::xplmType_Data as _,
        TestDataPayload::Refd(dr) => dr.typ,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMFindDataRef(name: *const c_char) -> XPLMDataRef {
    let name = unsafe { CStr::from_ptr(name) };
    let mut datarefs = STUB_DATAREFS.lock().unwrap();
    let datarefs = datarefs.get_or_insert_with(create_test_drs);
    let Some(dr) = datarefs.get(name) else {
        return std::ptr::null_mut();
    };
    let dr_ptr: *const TestData = &**dr;
    dr_ptr as *mut c_void
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMCanWriteDataRef(_: XPLMDataRef) -> c_int {
    1
}

macro_rules! impl_scalar_dr_accessors {
    (
        $getter:ident,
        $setter:ident,
        $c_type:ty,
        $variant:ident,
        $xp_type:expr,
        $read_field:ident,
        $write_field:ident
        $(,)?
    ) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $getter(dr: XPLMDataRef) -> $c_type {
            let dr = unsafe { (dr as *const TestData).as_ref().unwrap() };
            match &dr.data {
                TestDataPayload::$variant(inner_value) => *inner_value,
                TestDataPayload::Refd(dr) => {
                    if (dr.typ as u32 & $xp_type) != 0 {
                        if let Some(read_cb) = dr.$read_field {
                            unsafe { read_cb(dr.read_refcon) }
                        } else {
                            panic!("dataref {dr:?} is missing a read callback");
                        }
                    } else {
                        panic!("attempted to {} from dataref {dr:?}", stringify!($getter));
                    }
                }
                _ => panic!("attempted to {} from dataref {dr:?}", stringify!($getter)),
            }
        }
        #[unsafe(no_mangle)]
        pub extern "C" fn $setter(dr: XPLMDataRef, value: $c_type) {
            let dr = unsafe { (dr as *mut TestData).as_mut().unwrap() };
            match &mut dr.data {
                TestDataPayload::$variant(inner_value) => *inner_value = value,
                TestDataPayload::Refd(dr) => {
                    if (dr.typ as u32 & $xp_type) == 0 {
                        panic!("attempted to {} from dataref {dr:?}", stringify!($setter));
                    } else if !dr.writable {
                        panic!("attempted to write to read-only dataref {dr:?}",);
                    } else if let Some(write_cb) = dr.$write_field {
                        unsafe { write_cb(dr.write_refcon, value) }
                    } else {
                        panic!("dataref {dr:?} is missing a write callback");
                    }
                }
                _ => panic!("attempted to write {value:?} into dataref {dr:?}"),
            }
        }
    };
}

impl_scalar_dr_accessors!(
    XPLMGetDatai,
    XPLMSetDatai,
    c_int,
    OwnedI32,
    xplmType_Int,
    read_i32,
    write_i32,
);
impl_scalar_dr_accessors!(
    XPLMGetDataf,
    XPLMSetDataf,
    c_float,
    OwnedF32,
    xplmType_Float,
    read_f32,
    write_f32,
);
impl_scalar_dr_accessors!(
    XPLMGetDatad,
    XPLMSetDatad,
    c_double,
    OwnedF64,
    xplmType_Double,
    read_f64,
    write_f64,
);

macro_rules! impl_vector_dr_accessors {
    (
        $getter:ident,
        $setter:ident,
        $c_type:ty,
        $variant:ident,
        $xp_type:expr,
        $read_field:ident,
        $write_field:ident
        $(,)?
    ) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $getter(
            dr: XPLMDataRef,
            dest: *mut $c_type,
            offset: c_int,
            out_cap: c_int,
        ) -> c_int {
            let dr = unsafe { (dr as *const TestData).as_ref().unwrap() };
            match &dr.data {
                TestDataPayload::$variant(src) => {
                    let start_offset = usize::try_from(offset).unwrap();
                    let dest = unsafe { slice::from_raw_parts_mut(dest as *mut _, out_cap as _) };
                    let copy_length = dest.len().min(src.len().saturating_sub(start_offset));
                    let end_offset = start_offset + copy_length;
                    dest[..copy_length].copy_from_slice(&src[start_offset..end_offset]);
                    copy_length as _
                }
                TestDataPayload::Refd(dr) => {
                    if (dr.typ as u32 & $xp_type) != 0 {
                        if let Some(read_cb) = dr.$read_field {
                            unsafe { read_cb(dr.read_refcon, dest, offset, out_cap) }
                        } else {
                            panic!("dataref {dr:?} is missing a read callback");
                        }
                    } else {
                        panic!("attempted to {} from dataref {dr:?}", stringify!($getter));
                    }
                }
                _ => panic!("attempted to {} from dataref {dr:?}", stringify!($getter)),
            }
        }

        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $setter(
            dr: XPLMDataRef,
            src: *mut $c_type,
            offset: c_int,
            in_count: c_int,
        ) {
            let dr = unsafe { (dr as *mut TestData).as_mut().unwrap() };
            match &mut dr.data {
                TestDataPayload::$variant(dest) => {
                    let start_offset = usize::try_from(offset).unwrap();
                    let src = unsafe { slice::from_raw_parts(src as *const _, in_count as _) };
                    let copy_length = src.len().min(dest.len().saturating_sub(start_offset));
                    let end_offset = start_offset + copy_length;
                    dest[start_offset..end_offset].copy_from_slice(&src[..copy_length]);
                }
                TestDataPayload::Refd(dr) => {
                    if (dr.typ as u32 & $xp_type) == 0 {
                        panic!("attempted to {} from dataref {dr:?}", stringify!($setter));
                    } else if !dr.writable {
                        panic!("attempted to write to read-only dataref {dr:?}",);
                    } else if let Some(write_cb) = dr.$write_field {
                        unsafe { write_cb(dr.write_refcon, src, offset, in_count) }
                    } else {
                        panic!("dataref {dr:?} is missing a write callback");
                    }
                }
                _ => panic!("attempted to {} into dataref {dr:?}", stringify!($setter)),
            }
        }
    };
}

impl_vector_dr_accessors!(
    XPLMGetDatavi,
    XPLMSetDatavi,
    c_int,
    OwnedI32Array,
    xplmType_IntArray,
    read_i32_array,
    write_i32_array,
);
impl_vector_dr_accessors!(
    XPLMGetDatavf,
    XPLMSetDatavf,
    c_float,
    OwnedF32Array,
    xplmType_FloatArray,
    read_f32_array,
    write_f32_array,
);
impl_vector_dr_accessors!(
    XPLMGetDatab,
    XPLMSetDatab,
    c_void,
    OwnedByteArray,
    xplmType_Data,
    read_data,
    write_data,
);

#[allow(trivial_casts)]
#[unsafe(no_mangle)]
pub extern "C" fn XPLMRegisterDataAccessor(
    name: *const c_char,
    typ: XPLMDataTypeID,
    writable: c_int,
    read_i32: XPLMGetDatai_f,
    write_i32: XPLMSetDatai_f,
    read_f32: XPLMGetDataf_f,
    write_f32: XPLMSetDataf_f,
    read_f64: XPLMGetDatad_f,
    write_f64: XPLMSetDatad_f,
    read_i32_array: XPLMGetDatavi_f,
    write_i32_array: XPLMSetDatavi_f,
    read_f32_array: XPLMGetDatavf_f,
    write_f32_array: XPLMSetDatavf_f,
    read_data: XPLMGetDatab_f,
    write_data: XPLMSetDatab_f,
    read_refcon: *mut c_void,
    write_refcon: *mut c_void,
) -> XPLMDataRef {
    let mut datarefs = STUB_DATAREFS.lock().unwrap();
    let datarefs = datarefs.get_or_insert_with(create_test_drs);

    assert!(!name.is_null());
    let name = unsafe { CStr::from_ptr(name).to_owned() };

    let dataref = TestData {
        name: name.clone(),
        data: TestDataPayload::Refd(TestDataRef {
            typ,
            writable: writable != 0,
            read_i32,
            write_i32,
            read_f32,
            write_f32,
            read_f64,
            write_f64,
            read_i32_array,
            write_i32_array,
            read_f32_array,
            write_f32_array,
            read_data,
            write_data,
            read_refcon,
            write_refcon,
        }),
    };
    let dataref = Box::pin(dataref);
    assert!(!datarefs.contains_key(&name));
    let test_data: *mut _ = &mut **datarefs.entry(name).or_insert(dataref);
    test_data as *mut _
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMUnregisterDataAccessor(dr: XPLMDataRef) {
    assert!(!dr.is_null());
    let dr = dr as *const TestData;
    let dr = unsafe { dr.as_ref().unwrap() };
    let name = dr.name.clone();

    let mut datarefs = STUB_DATAREFS.lock().unwrap();
    let datarefs = datarefs.get_or_insert_with(create_test_drs);
    assert!(datarefs.contains_key(&name));
    datarefs.remove(&name);
}

// Used by the DRE/DRT notification logic in datarefs. Don't return anything.
#[unsafe(no_mangle)]
pub extern "C" fn XPLMFindPluginBySignature(_: *const c_char) -> XPLMPluginID {
    XPLM_NO_PLUGIN_ID
}

// Used by the DRE/DRT notification logic in datarefs. Ignore the message.
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSendMessageToPlugin(_: XPLMPluginID, _: c_int, _: *mut c_void) {}
