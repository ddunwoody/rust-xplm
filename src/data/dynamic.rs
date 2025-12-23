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

use std::{
    ffi::{c_void, CString},
    marker::PhantomData,
    ptr::NonNull,
};

use xplm_sys::{
    XPLMDataRef, XPLMFindDataRef, XPLMGetDatab_f, XPLMGetDatad_f, XPLMGetDataf_f, XPLMGetDatai_f,
    XPLMGetDatavf_f, XPLMGetDatavi_f, XPLMRegisterDataAccessor, XPLMSetDatab_f, XPLMSetDatad_f,
    XPLMSetDataf_f, XPLMSetDatai_f, XPLMSetDatavf_f, XPLMSetDatavi_f, XPLMUnregisterDataAccessor,
};

use crate::data::{
    owned::{notify_dre_plugin, CreateError},
    Access, ArrayType, DataType, ReadOnly, ReadWrite,
};

/// Trait which must be implemented by data sources for reading dynamic scalar datarefs.
pub trait DynamicDataRead<T: DataType + ?Sized> {
    /// Called by the dataref to read the dataref's scalar data.
    fn read(&self) -> T;
}

/// Trait which must be implemented by data sources for writing dynamic scalar datarefs.
pub trait DynamicDataReadWrite<T: DataType + ?Sized>: DynamicDataRead<T> {
    /// Called by the dataref to write new scalar data into the dataref.
    fn write(&mut self, value: T);
}

/// Trait which must be implemented by data sources for reading dynamic array datarefs.
#[allow(clippy::len_without_is_empty)]
pub trait DynamicArrayRead<T: DataType> {
    /// Called by the dataref to read the dataref's array data. The dataref should be read
    /// starting at `offset` and the data should be written to the `out_data` slice. The
    /// function should return the actual number of data items written.
    fn read(&self, out_data: &mut [T], offset: usize) -> usize;
    /// Should return the full length of the array dataref.
    fn len(&self) -> usize;
}

/// Trait which must be implemented by data sources for writing dynamic array datarefs.
pub trait DynamicArrayReadWrite<T: DataType>: DynamicArrayRead<T> {
    /// Called by the dataref to write new array data into the dataref. The data should
    /// be written starting at `offset`.
    fn write(&mut self, in_data: &[T], offset: usize);
}

/// An owned dataref whose contents are dynamically generated. You can use this to create
/// fully dynamic datarefs, where the data contents are constructed on the fly with from
/// any additional context you might require.
///
/// # Example
/// ```no_run
/// use xplm::data::dynamic::{DynamicDataRead, DynamicData};
///
/// // Our dynamic data source.
/// struct MyUnixTime {}
/// // To allow reading, we must implement either the `DynamicDataRead` trait for scalar
/// // data types, or the `DynamicArrayRead` trait for array data respectively. If the
/// // dataref was created with `ReadWrite` access, we would also need to implement either
/// // the `DynamicDataReadWrite` or `DynamicArrayReadWrite` trait, as appropriate for the
/// // dataref's type.
/// impl DynamicDataRead<f64> for MyUnixTime {
///     fn read(&self) -> f64 {
///         std::time::SystemTime::now()
///             .duration_since(std::time::UNIX_EPOCH)
///             .unwrap()
///             .as_secs_f64()
///     }
/// }
/// // Creates the dataref. Any reads of the dataref will now call the `read' function above
/// // to retrieve the value.
/// let dr = DynamicData::<f64, MyUnixTime>::create("my/unix_time", MyUnixTime {}).unwrap();
/// ```
pub struct DynamicData<T: DataType + ?Sized, D, A = ReadOnly> {
    /// The dataref handle
    id: XPLMDataRef,
    /// The source for the data dynamically produced by this dataref. This must be
    /// boxed to allow passing a permanent pointer to it to the dataref registration
    /// system as the `refcon` argument. This way, the address of the contents will
    /// not move. We don't need `std::pin::Pin` here, because the `DynamicData` is
    /// not self-referrential.
    _data_source: Box<D>,
    /// Payload phantom data
    data_phantom: PhantomData<T>,
    /// Data access phantom data
    access_phantom: PhantomData<A>,
}

// We must not forget to de-register the data accessor from X-Plane when we're dropped.
impl<T: DataType + ?Sized, D, A> Drop for DynamicData<T, D, A> {
    fn drop(&mut self) {
        unsafe {
            assert!(!self.id.is_null());
            XPLMUnregisterDataAccessor(self.id);
        }
    }
}

impl<T: DataType + ?Sized, D, A: Access> DynamicData<T, D, A> {
    fn create_common(
        name: &str,
        data_source: D,
        cbs: CallbackSet<T, D, A>,
    ) -> Result<Self, CreateError> {
        let name_c = CString::new(name)?;

        // Fail if the dataref already exists
        let existing = unsafe { XPLMFindDataRef(name_c.as_ptr()) };
        if !existing.is_null() {
            return Err(CreateError::Exists);
        }
        // Box the data source and use the box pointer as the refcon. This won't
        // move as long as the dataref exists, so we can safely rely on it.
        let data_source = Box::new(data_source);
        let refcon: *const D = &*data_source;
        let id = unsafe {
            XPLMRegisterDataAccessor(
                name_c.as_ptr(),
                T::sim_type(),
                A::writeable() as _,
                cbs.int_read,
                cbs.int_write,
                cbs.float_read,
                cbs.float_write,
                cbs.double_read,
                cbs.double_write,
                cbs.int_array_read,
                cbs.int_array_write,
                cbs.float_array_read,
                cbs.float_array_write,
                cbs.byte_array_read,
                cbs.byte_array_write,
                refcon as *mut _,
                refcon as *mut _,
            )
        };
        if id.is_null() {
            return Err(CreateError::RegisterFailed);
        }
        notify_dre_plugin(&name_c);
        Ok(Self {
            id,
            _data_source: data_source,
            data_phantom: PhantomData,
            access_phantom: PhantomData,
        })
    }
}

macro_rules! impl_dynamic_data {
    ($native_type:ty) => {
        impl<D: DynamicDataRead<$native_type>> DynamicData<$native_type, D, ReadOnly> {
            /// Creates a new read-only scalar dataref with a dynamic data source.
            pub fn create(name: &str, data_source: D) -> Result<Self, CreateError> {
                Self::create_common(
                    name,
                    data_source,
                    CallbackSet::<$native_type, D, ReadOnly>::new(),
                )
            }
        }
        impl<D: DynamicDataReadWrite<$native_type>> DynamicData<$native_type, D, ReadWrite> {
            /// Creates a new writeable scalar dataref with a dynamic data source.
            pub fn create(name: &str, data_source: D) -> Result<Self, CreateError> {
                Self::create_common(
                    name,
                    data_source,
                    CallbackSet::<$native_type, D, ReadWrite>::new(),
                )
            }
        }
    };
    (array $native_type:ty) => {
        impl<D: DynamicArrayRead<$native_type>> DynamicData<[$native_type], D, ReadOnly>
        where
            [$native_type]: ArrayType,
        {
            /// Creates a new read-only array dataref with a dynamic data source.
            pub fn create(name: &str, data_source: D) -> Result<Self, CreateError> {
                Self::create_common(
                    name,
                    data_source,
                    CallbackSet::<[$native_type], D, ReadOnly>::new(),
                )
            }
        }
        impl<D: DynamicArrayReadWrite<$native_type>> DynamicData<[$native_type], D, ReadWrite>
        where
            [$native_type]: ArrayType,
        {
            /// Creates a new writeable array dataref with a dynamic data source.
            pub fn create(name: &str, data_source: D) -> Result<Self, CreateError> {
                Self::create_common(
                    name,
                    data_source,
                    CallbackSet::<[$native_type], D, ReadWrite>::new(),
                )
            }
        }
    };
}

impl_dynamic_data!(i32);
impl_dynamic_data!(f32);
impl_dynamic_data!(f64);

impl_dynamic_data!(array u8);
impl_dynamic_data!(array i32);
impl_dynamic_data!(array f32);

struct CallbackSet<T: ?Sized, D, A: Access> {
    int_read: XPLMGetDatai_f,
    int_write: XPLMSetDatai_f,
    float_read: XPLMGetDataf_f,
    float_write: XPLMSetDataf_f,
    double_read: XPLMGetDatad_f,
    double_write: XPLMSetDatad_f,
    int_array_read: XPLMGetDatavi_f,
    int_array_write: XPLMSetDatavi_f,
    float_array_read: XPLMGetDatavf_f,
    float_array_write: XPLMSetDatavf_f,
    byte_array_read: XPLMGetDatab_f,
    byte_array_write: XPLMSetDatab_f,
    data_phantom: PhantomData<T>,
    data_source: PhantomData<D>,
    access_phantom: PhantomData<A>,
}

#[inline]
fn i32_to_usize(value: i32, name: &'static str) -> usize {
    usize::try_from(value)
        .unwrap_or_else(|e| panic!("invalid {name} value {value} encountered: {e}"))
}

macro_rules! impl_callback_set {
    ($native_type:ty, $read:ident, $write:ident) => {
        impl<D: DynamicDataRead<$native_type>> CallbackSet<$native_type, D, ReadOnly> {
            fn new() -> Self {
                Self {
                    $read: Some(Self::$read),
                    ..Default::default()
                }
            }
            extern "C" fn $read(refcon: *mut c_void) -> $native_type {
                let data_source = NonNull::new(refcon as *mut D)
                    .expect(concat!(stringify!($read), " called with NULL refcon"));
                unsafe { data_source.as_ref().read() }
            }
        }
        impl<D: DynamicDataReadWrite<$native_type>> CallbackSet<$native_type, D, ReadWrite> {
            fn new() -> Self {
                Self {
                    $read: Some(CallbackSet::<$native_type, D, ReadOnly>::$read),
                    $write: Some(Self::$write),
                    ..Default::default()
                }
            }
            extern "C" fn $write(refcon: *mut c_void, value: $native_type) {
                let mut refcon = NonNull::new(refcon as *mut D)
                    .expect(concat!(stringify!($read), " called with NULL refcon"));
                unsafe { refcon.as_mut().write(value) };
            }
        }
    };
    (array $native_type:ty, $c_type:ty, $read:ident, $write:ident) => {
        impl<D: DynamicArrayRead<$native_type>> CallbackSet<[$native_type], D, ReadOnly> {
            fn new() -> Self {
                Self {
                    $read: Some(Self::$read),
                    ..Default::default()
                }
            }
            extern "C" fn $read(
                refcon: *mut c_void,
                out_data: *mut $c_type,
                offset: i32,
                count: i32,
            ) -> i32 {
                let refcon = NonNull::new(refcon as *mut D)
                    .expect(concat!(stringify!($read), " called with NULL refcon"));
                let offset = i32_to_usize(offset, "offset");
                let count = i32_to_usize(count, "count");
                let refcon = unsafe { refcon.as_ref() };
                if let Some(values) = NonNull::new(out_data as *mut $native_type) {
                    assert!(values.is_aligned());
                    let data = unsafe { std::slice::from_raw_parts_mut(values.as_ptr(), count) };
                    refcon.read(data, offset) as _
                } else {
                    refcon.len() as _
                }
            }
        }
        impl<D: DynamicArrayReadWrite<$native_type>> CallbackSet<[$native_type], D, ReadWrite> {
            fn new() -> Self {
                Self {
                    $read: Some(CallbackSet::<[$native_type], D, ReadOnly>::$read),
                    $write: Some(Self::$write),
                    ..Default::default()
                }
            }
            extern "C" fn $write(
                refcon: *mut c_void,
                in_data: *mut $c_type,
                offset: i32,
                count: i32,
            ) {
                let mut refcon = NonNull::new(refcon as *mut D)
                    .expect(concat!(stringify!($write), " called with NULL refcon"));
                let offset = i32_to_usize(offset, "offset");
                let count = i32_to_usize(count, "count");
                let refcon = unsafe { refcon.as_mut() };
                if let Some(values) = NonNull::new(in_data as *mut $native_type) {
                    assert!(values.is_aligned());
                    let data = unsafe { std::slice::from_raw_parts(values.as_ptr(), count) };
                    refcon.write(data, offset)
                }
            }
        }
    };
}

impl<T: ?Sized, D, A: Access> Default for CallbackSet<T, D, A> {
    fn default() -> Self {
        Self {
            int_read: None,
            int_write: None,
            float_read: None,
            float_write: None,
            double_read: None,
            double_write: None,
            int_array_read: None,
            int_array_write: None,
            float_array_read: None,
            float_array_write: None,
            byte_array_read: None,
            byte_array_write: None,
            data_phantom: PhantomData,
            data_source: PhantomData,
            access_phantom: PhantomData,
        }
    }
}

impl_callback_set!(i32, int_read, int_write);
impl_callback_set!(f32, float_read, float_write);
impl_callback_set!(f64, double_read, double_write);
impl_callback_set!(array i32, i32, int_array_read, int_array_write);
impl_callback_set!(array f32, f32, float_array_read, float_array_write);
impl_callback_set!(array u8, c_void, byte_array_read, byte_array_write);

#[cfg(test)]
mod tests {
    use crate::data::borrowed::DataRef;
    use crate::data::dynamic::{
        DynamicArrayRead, DynamicArrayReadWrite, DynamicData, DynamicDataRead, DynamicDataReadWrite,
    };
    use crate::data::{ArrayRead, ArrayReadWrite, DataRead, DataReadWrite, ReadWrite};
    macro_rules! gen_dyn_test {
        ($test_func:ident, $native_type:ty, $init_value:literal, $set_value:literal$(,)?) => {
            #[test]
            fn $test_func() {
                struct TestData {
                    value: $native_type,
                }
                impl DynamicDataRead<$native_type> for TestData {
                    fn read(&self) -> $native_type {
                        self.value
                    }
                }
                impl DynamicDataReadWrite<$native_type> for TestData {
                    fn write(&mut self, value: $native_type) {
                        self.value = value;
                    }
                }
                let _dr_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();
                let _dr_dyn = DynamicData::<$native_type, _, ReadWrite>::create(
                    concat!("test/dyn/", stringify!($native_type)),
                    TestData { value: $init_value },
                );
                let mut dr_ref = DataRef::<$native_type, _>::find(concat!(
                    "test/dyn/",
                    stringify!($native_type)
                ))
                .unwrap()
                .writeable()
                .unwrap();
                assert_eq!(dr_ref.get(), $init_value);
                dr_ref.set($set_value);
                assert_eq!(dr_ref.get(), $set_value);
            }
        };
        ($test_func:ident, array [$native_type:ty], $init_values:expr, $set_values:expr$(,)?) => {
            #[test]
            fn $test_func() {
                struct TestData {
                    values: [$native_type; 4],
                }
                impl DynamicArrayRead<$native_type> for TestData {
                    fn read(&self, out_data: &mut [$native_type], offset: usize) -> usize {
                        let to_copy = self.values.len().saturating_sub(offset).min(out_data.len());
                        out_data[..to_copy]
                            .copy_from_slice(&self.values[offset..(offset + to_copy)]);
                        to_copy
                    }
                    fn len(&self) -> usize {
                        self.values.len()
                    }
                }
                impl DynamicArrayReadWrite<$native_type> for TestData {
                    fn write(&mut self, in_data: &[$native_type], offset: usize) {
                        let to_copy = self.values.len().saturating_sub(offset).min(in_data.len());
                        self.values[offset..(offset + to_copy)]
                            .copy_from_slice(&in_data[..to_copy]);
                    }
                }
                let _dr_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();
                let _dr_dyn = DynamicData::<[$native_type], _, ReadWrite>::create(
                    concat!("test/dyn/", stringify!($native_type), "_array"),
                    TestData {
                        values: $init_values,
                    },
                );
                let mut dr_ref = DataRef::<[$native_type], _>::find(concat!(
                    "test/dyn/",
                    stringify!($native_type),
                    "_array",
                ))
                .unwrap()
                .writeable()
                .unwrap();
                // Check length reading
                assert_eq!(dr_ref.len(), $init_values.len());

                // Check that the initial values are found in the dataref
                let mut values = [<$native_type>::default(); $init_values.len()];
                assert_eq!(dr_ref.get(&mut values), $init_values.len());
                assert_eq!(values, $init_values);

                // Check for correct out-of-bounds reading behavior. Slide the read window
                // by 1 element to the right, and check that the appropriate number of elements
                // is read, up the full length of the array (when zero should be returned).
                for offset in 1..=$init_values.len() {
                    assert_eq!(
                        dr_ref.get_subdata(&mut values, offset),
                        $init_values.len() - offset
                    );
                }

                // Check that we can set new values and read them back
                dr_ref.set(&$set_values);
                let mut values = [<$native_type>::default(); $set_values.len()];
                assert_eq!(dr_ref.get(&mut values), $set_values.len());
                assert_eq!(values, $set_values);
            }
        };
    }

    gen_dyn_test!(test_dyn_i32, i32, 1234, 5678);
    gen_dyn_test!(test_dyn_f32, f32, 1234.0, 5678.0);
    gen_dyn_test!(test_dyn_f64, f64, 1234.0, 5678.0);

    gen_dyn_test!(test_dyn_u8_array, array[u8], [1, 2, 3, 4], [5, 6, 7, 8]);
    gen_dyn_test!(test_dyn_i32_array, array[i32], [1, 2, 3, 4], [5, 6, 7, 8]);
    gen_dyn_test!(
        test_dyn_f32_array,
        array[f32],
        [1.0, 2.0, 3.0, 4.0],
        [5.0, 6.0, 7.0, 8.0],
    );
}
