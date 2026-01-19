use super::{Access, ArrayRead, ArrayReadWrite, DataRead, DataReadWrite, DataType, ReadOnly};
use std::cmp;
use std::ffi::{c_double, c_float, CString, NulError};
use std::marker::PhantomData;
use std::os::raw::{c_int, c_void};
use xplm_sys::*;

/// A dataref owned by this plugin
///
/// The access parameter of this type determines whether X-Plane and other plugins can write
/// this dataref. Owned datarefs can always be written by this plugin.
#[derive(Debug)]
pub struct OwnedData<T: DataType + OwnedDataType + ?Sized, A = ReadOnly> {
    /// The dataref handle
    id: XPLMDataRef,
    /// The current value
    ///
    /// This is boxed so that it will have a constant memory location that is
    /// provided as a refcon to the callbacks.
    value: Box<T::Storage>,
    /// Data access phantom data
    access_phantom: PhantomData<A>,
}

impl<T: DataType + OwnedDataType + ?Sized, A: Access> OwnedData<T, A> {
    /// Creates a new dataref with the provided name containing the default value of T
    pub fn create(name: &str) -> Result<Self, CreateError>
    where
        T: Default,
    {
        Self::create_with_value(name, &T::default())
    }

    /// Creates a new dataref with the provided name and value
    pub fn create_with_value(name: &str, value: &T) -> Result<Self, CreateError> {
        let name_c = CString::new(name)?;

        let existing = unsafe { XPLMFindDataRef(name_c.as_ptr()) };
        if !existing.is_null() {
            return Err(CreateError::Exists);
        }

        let value = value.to_storage();
        let mut value_box = Box::new(value);
        let value_ptr: *mut T::Storage = value_box.as_mut();

        let id = unsafe {
            XPLMRegisterDataAccessor(
                name_c.as_ptr(),
                T::sim_type(),
                Self::writeable(),
                T::int_read(),
                T::int_write(),
                T::float_read(),
                T::float_write(),
                T::double_read(),
                T::double_write(),
                T::int_array_read(),
                T::int_array_write(),
                T::float_array_read(),
                T::float_array_write(),
                T::byte_array_read(),
                T::byte_array_write(),
                value_ptr as *mut c_void,
                value_ptr as *mut c_void,
            )
        };
        if id.is_null() {
            return Err(CreateError::RegisterFailed);
        }
        notify_dre_plugin(&name_c);
        Ok(OwnedData {
            id,
            value: value_box,
            access_phantom: PhantomData,
        })
    }

    /// Returns 1 if this dataref should be writeable by other plugins and X-Plane
    fn writeable() -> i32 {
        if A::writeable() {
            1
        } else {
            0
        }
    }
}

impl<T: DataType + OwnedDataType + ?Sized, A> Drop for OwnedData<T, A> {
    fn drop(&mut self) {
        unsafe { XPLMUnregisterDataAccessor(self.id) }
    }
}

pub trait OwnedDataType {
    fn int_read() -> XPLMGetDatai_f {
        None
    }
    fn int_write() -> XPLMSetDatai_f {
        None
    }
    fn float_read() -> XPLMGetDataf_f {
        None
    }
    fn float_write() -> XPLMSetDataf_f {
        None
    }
    fn double_read() -> XPLMGetDatad_f {
        None
    }
    fn double_write() -> XPLMSetDatad_f {
        None
    }
    fn int_array_read() -> XPLMGetDatavi_f {
        None
    }
    fn int_array_write() -> XPLMSetDatavi_f {
        None
    }
    fn float_array_read() -> XPLMGetDatavf_f {
        None
    }
    fn float_array_write() -> XPLMSetDatavf_f {
        None
    }
    fn byte_array_read() -> XPLMGetDatab_f {
        None
    }
    fn byte_array_write() -> XPLMSetDatab_f {
        None
    }
}

macro_rules! impl_scalar_read_write {
    ($native_type:ty, $c_type:ty, $read_fn:ident, $read_fn_type:ty, $write_fn:ident,
    $write_fn_type:ty$(,)?) => {
        impl OwnedDataType for $native_type {
            fn $read_fn() -> $read_fn_type {
                unsafe extern "C" fn read_fn(refcon: *mut c_void) -> $c_type {
                    let storage = refcon as *const $native_type;
                    (*storage) as $c_type
                }
                Some(read_fn)
            }
            fn $write_fn() -> $write_fn_type {
                unsafe extern "C" fn write_fn(refcon: *mut c_void, value: $c_type) {
                    let storage = refcon as *mut $native_type;
                    (*storage) = value as $native_type;
                }
                Some(write_fn)
            }
        }
    };
}

macro_rules! impl_array_read_write {
    ($native_type:ty, $c_type:ty, $read_fn:ident, $read_fn_type:ty, $write_fn:ident,
    $write_fn_type:ty$(,)?) => {
        impl OwnedDataType for [$native_type] {
            fn $read_fn() -> $read_fn_type {
                unsafe extern "C" fn read_fn(
                    refcon: *mut c_void,
                    out_values: *mut $c_type,
                    offset: c_int,
                    len: c_int,
                ) -> c_int {
                    let vec = refcon as *mut Vec<$native_type>;
                    let vec = unsafe { vec.as_mut().expect("null pointer encountered") };
                    let Ok(len) = usize::try_from(len) else {
                        return 0;
                    };
                    let Ok(offset) = usize::try_from(offset) else {
                        return 0;
                    };
                    if out_values.is_null() {
                        return vec.len() as _;
                    }
                    let out_values = out_values as *mut $native_type;
                    let out_values = unsafe { std::slice::from_raw_parts_mut(out_values, len) };
                    if offset >= vec.len() {
                        return 0;
                    }
                    let to_copy = (vec.len() - offset).min(len);
                    out_values[..to_copy].copy_from_slice(&vec[offset..(offset + to_copy)]);
                    to_copy as c_int
                }
                Some(read_fn)
            }
            fn $write_fn() -> $write_fn_type {
                unsafe extern "C" fn write_fn(
                    refcon: *mut c_void,
                    in_values: *mut $c_type,
                    offset: c_int,
                    len: c_int,
                ) {
                    if in_values.is_null() {
                        return;
                    }
                    let vec = refcon as *mut Vec<$native_type>;
                    let vec = unsafe { vec.as_mut().expect("null pointer encountered") };
                    let Ok(len) = usize::try_from(len) else {
                        return;
                    };
                    let Ok(offset) = usize::try_from(offset) else {
                        return;
                    };
                    let in_values = in_values as *const $native_type;
                    let in_values = unsafe { std::slice::from_raw_parts(in_values, len) };
                    if offset >= vec.len() {
                        return;
                    }
                    let to_copy = (vec.len() - offset).min(len);
                    vec[offset..(offset + to_copy)].copy_from_slice(&in_values[..to_copy]);
                }
                Some(write_fn)
            }
        }
    };
}

impl OwnedDataType for bool {
    fn int_read() -> XPLMGetDatai_f {
        unsafe extern "C" fn read_fn(refcon: *mut c_void) -> c_int {
            let storage = refcon as *const bool;
            (*storage) as c_int
        }
        Some(read_fn)
    }
    fn int_write() -> XPLMSetDatai_f {
        unsafe extern "C" fn write_fn(refcon: *mut c_void, value: c_int) {
            let storage = refcon as *mut bool;
            (*storage) = value != 0;
        }
        Some(write_fn)
    }
}

impl_scalar_read_write!(
    i8,
    c_int,
    int_read,
    XPLMGetDatai_f,
    int_write,
    XPLMSetDatai_f,
);
impl_scalar_read_write!(
    u8,
    c_int,
    int_read,
    XPLMGetDatai_f,
    int_write,
    XPLMSetDatai_f,
);
impl_scalar_read_write!(
    i16,
    c_int,
    int_read,
    XPLMGetDatai_f,
    int_write,
    XPLMSetDatai_f,
);
impl_scalar_read_write!(
    u16,
    c_int,
    int_read,
    XPLMGetDatai_f,
    int_write,
    XPLMSetDatai_f,
);
impl_scalar_read_write!(
    i32,
    c_int,
    int_read,
    XPLMGetDatai_f,
    int_write,
    XPLMSetDatai_f,
);
impl_scalar_read_write!(
    f32,
    c_float,
    float_read,
    XPLMGetDataf_f,
    float_write,
    XPLMSetDataf_f,
);
impl_scalar_read_write!(
    f64,
    c_double,
    double_read,
    XPLMGetDatad_f,
    double_write,
    XPLMSetDatad_f,
);

impl_array_read_write!(
    i32,
    c_int,
    int_array_read,
    XPLMGetDatavi_f,
    int_array_write,
    XPLMSetDatavi_f,
);
impl_array_read_write!(
    f32,
    c_float,
    float_array_read,
    XPLMGetDatavf_f,
    float_array_write,
    XPLMSetDatavf_f,
);
impl_array_read_write!(
    u8,
    c_void,
    byte_array_read,
    XPLMGetDatab_f,
    byte_array_write,
    XPLMSetDatab_f,
);

impl OwnedDataType for [bool] {
    fn int_array_read() -> XPLMGetDatavi_f {
        unsafe extern "C" fn read_fn(
            refcon: *mut c_void,
            out_values: *mut c_int,
            offset: c_int,
            len: c_int,
        ) -> c_int {
            let vec = refcon as *mut Vec<bool>;
            let vec = unsafe { vec.as_mut().expect("null pointer encountered") };
            let Ok(len) = usize::try_from(len) else {
                return 0;
            };
            let Ok(offset) = usize::try_from(offset) else {
                return 0;
            };
            if out_values.is_null() {
                return vec.len() as _;
            }
            let out_values = unsafe { std::slice::from_raw_parts_mut(out_values, len) };
            if offset >= vec.len() {
                return 0;
            }
            let to_copy = (vec.len() - offset).min(len);
            for i in 0..to_copy {
                out_values[i] = vec[offset + i] as c_int;
            }
            to_copy as c_int
        }
        Some(read_fn)
    }
    fn int_array_write() -> XPLMSetDatavi_f {
        unsafe extern "C" fn write_fn(
            refcon: *mut c_void,
            in_values: *mut c_int,
            offset: c_int,
            len: c_int,
        ) {
            if in_values.is_null() {
                return;
            }
            let vec = refcon as *mut Vec<bool>;
            let vec = unsafe { vec.as_mut().expect("null pointer encountered") };
            let Ok(len) = usize::try_from(len) else {
                return;
            };
            let Ok(offset) = usize::try_from(offset) else {
                return;
            };
            let in_values = in_values as *const c_int;
            let in_values = unsafe { std::slice::from_raw_parts(in_values, len) };
            if offset >= vec.len() {
                return;
            }
            let to_copy = (vec.len() - offset).min(len);
            for i in 0..to_copy {
                vec[offset + i] = in_values[i] != 0;
            }
        }
        Some(write_fn)
    }
}

// Notifies DataRefEditor or DataRefTool about a newly created dataref, by sending an
// inter-plugin message containing the new dataref's name.
pub(crate) fn notify_dre_plugin(name_c: &std::ffi::CStr) {
    use std::cell::OnceCell;
    use std::sync::Mutex;
    const DRE_PLUGIN_SIGS: &[&std::ffi::CStr] = &[
        c"com.leecbaker.datareftool",
        c"xplanesdk.examples.DataRefEditor",
    ];
    const DRE_MSG_ADD_DATAREF: i32 = 0x01000000;
    static DATAREF_EDITOR: Mutex<OnceCell<Option<XPLMPluginID>>> = Mutex::new(OnceCell::new());

    // Check if DataRefEditor is present. If it is, notify it of the new dataref.
    if let Some(plugin_id) = DATAREF_EDITOR
        .lock()
        .expect("panicked lock")
        .get_or_init(|| unsafe {
            for sig in DRE_PLUGIN_SIGS {
                let plugin_id = XPLMFindPluginBySignature(sig.as_ptr());
                if plugin_id != XPLM_NO_PLUGIN_ID {
                    return Some(plugin_id);
                }
            }
            None
        })
        .as_ref()
        .copied()
    {
        unsafe {
            XPLMSendMessageToPlugin(
                plugin_id,
                DRE_MSG_ADD_DATAREF,
                name_c.as_ptr() as *mut c_void,
            );
        }
    }
}

// DataRead and DataReadWrite
macro_rules! impl_read_write {
    (for $native_type:ty) => {
        impl<A> DataRead<$native_type> for OwnedData<$native_type, A> {
            fn get(&self) -> $native_type {
                *self.value
            }
        }
        impl<A> DataReadWrite<$native_type> for OwnedData<$native_type, A> {
            fn set(&mut self, value: $native_type) {
                *self.value = value;
            }
        }
    };
    (for array [$native_type:ty]) => {
        impl<A> ArrayRead<[$native_type]> for OwnedData<[$native_type], A> {
            fn get(&self, dest: &mut [$native_type]) -> usize {
                let copy_length = cmp::min(dest.len(), self.value.len());
                let dest_sub = &mut dest[..copy_length];
                let value_sub = &self.value[..copy_length];
                dest_sub.copy_from_slice(value_sub);
                copy_length
            }
            fn get_subdata(&self, dest: &mut [$native_type], start_offset: usize) -> usize {
                let copy_length =
                    cmp::min(dest.len(), self.value.len().saturating_sub(start_offset));
                let dest_sub = &mut dest[..copy_length];
                let end_offset = start_offset + copy_length;
                let value_sub = &self.value[start_offset..end_offset];
                dest_sub.copy_from_slice(value_sub);
                copy_length
            }
            fn len(&self) -> usize {
                self.value.len()
            }
        }
        impl<A> ArrayReadWrite<[$native_type]> for OwnedData<[$native_type], A> {
            fn set(&mut self, values: &[$native_type]) {
                let copy_length = cmp::min(values.len(), self.value.len());
                let src_sub = &values[..copy_length];
                let values_sub = &mut self.value[..copy_length];
                values_sub.copy_from_slice(src_sub);
            }
            fn set_subdata(&mut self, values: &[$native_type], start_offset: usize) {
                let copy_length =
                    cmp::min(values.len(), self.value.len().saturating_sub(start_offset));
                let src_sub = &values[..copy_length];
                let end_offset = start_offset + copy_length;
                let values_sub = &mut self.value[start_offset..end_offset];
                values_sub.copy_from_slice(src_sub);
            }
        }
    };
}

impl_read_write!(for u8);
impl_read_write!(for i8);
impl_read_write!(for u16);
impl_read_write!(for i16);
impl_read_write!(for i32);
impl_read_write!(for f32);
impl_read_write!(for f64);
impl_read_write!(for bool);
impl_read_write!(for array [i32]);
impl_read_write!(for array [f32]);
impl_read_write!(for array [u8]);
impl_read_write!(for array [bool]);

/// Errors that can occur when creating a DataRef
#[derive(Clone, thiserror::Error, Debug)]
pub enum CreateError {
    /// The provided DataRef name contained a null byte
    #[error("Null byte in dataref name")]
    Null(#[from] NulError),

    /// The DataRef exists already
    #[error("DataRef already exists")]
    Exists,

    /// X-Plane failed creating the dataref (returned `NULL` from the register function).
    #[error("Registering the DataRef failed for an unknown reason")]
    RegisterFailed,
}

#[cfg(test)]
mod tests {
    use crate::data::{
        borrowed::DataRef,
        owned::{OwnedData, OwnedDataType},
        ArrayRead, ArrayReadWrite, ArrayType, DataRead, DataReadWrite, DataType, ReadWrite,
    };
    #[test]
    fn test_owned_data() -> Result<(), Box<dyn std::error::Error>> {
        let _dr_test_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();

        fn test_scalar<T>(name: &str, test_value: T)
        where
            T: Copy + DataType + OwnedDataType + Default + PartialEq + std::fmt::Debug,
            OwnedData<T, ReadWrite>: DataRead<T> + DataReadWrite<T>,
            DataRef<T>: DataRead<T>,
            DataRef<T, ReadWrite>: DataReadWrite<T>,
        {
            // Create and check that the default contents are present
            let mut dr: OwnedData<T, ReadWrite> = OwnedData::create(name).unwrap();
            assert_eq!(dr.get(), T::default());
            // Set the test value and validate read back
            dr.set(test_value);
            assert_eq!(dr.get(), test_value);
            // Grab a reference to the data and validate that the test value is still being read
            let mut dr_ref: DataRef<T, ReadWrite> =
                DataRef::find(name).unwrap().writeable().unwrap();
            assert_eq!(dr_ref.get(), test_value);
            // Set the default value again and validate by reading back
            dr_ref.set(T::default());
            assert_eq!(dr_ref.get(), T::default());
        }
        test_scalar::<i8>("test/owned/i8", i8::MIN);
        test_scalar::<i8>("test/owned/i8", i8::MAX);
        test_scalar::<u8>("test/owned/u8", u8::MAX);
        test_scalar::<i16>("test/owned/i16", i16::MIN);
        test_scalar::<i16>("test/owned/i16", i16::MAX);
        test_scalar::<u16>("test/owned/u16", u16::MAX);
        test_scalar::<i32>("test/owned/i32", i32::MIN);
        test_scalar::<i32>("test/owned/i32", i32::MAX);
        test_scalar::<f32>("test/owned/f32", 1.0);
        test_scalar::<f64>("test/owned/f64", 1.0);
        test_scalar::<bool>("test/owned/bool", true);

        fn test_array<T>(name: &str, test_value: &[<[T] as ArrayType>::Element])
        where
            [T]: ArrayType + OwnedDataType,
            <[T] as ArrayType>::Element: Copy + std::fmt::Debug + Default + PartialEq,
            [<[T] as ArrayType>::Element]: DataType + OwnedDataType,
            OwnedData<[<[T] as ArrayType>::Element], ReadWrite>: ArrayReadWrite<[T]>,
            DataRef<[<[T] as ArrayType>::Element], ReadWrite>: ArrayReadWrite<[T]>,
        {
            // Create the dataref with the test values already inserted
            let dr: OwnedData<[<[T] as ArrayType>::Element], ReadWrite> =
                OwnedData::create_with_value(name, test_value).unwrap();
            // Read back the test values to make sure they match
            assert_eq!(dr.as_vec(), test_value);

            // Grab a reference to the owned data
            let mut dr_ref: DataRef<[<[T] as ArrayType>::Element], ReadWrite> =
                DataRef::find(name).unwrap().writeable().unwrap();
            // Check that what's stored is still equal to the test value
            assert_eq!(dr_ref.as_vec(), test_value);
            // Insert a new array consisting of just the default values and read back to verify
            let def = vec![<[T] as ArrayType>::Element::default(); test_value.len()];
            dr_ref.set(&def);
            assert_eq!(dr_ref.as_vec(), def);
        }
        test_array::<i32>("test/owned/i32_array", &[1, 2, 3, 4]);
        test_array::<f32>("test/owned/f32_array", &[1.0, 2.0, 3.0, 4.0]);
        test_array::<u8>("test/owned/byte_array", "abcd".as_bytes());
        test_array::<bool>("test/owned/bool_array", &[true, false, true, false]);

        Ok(())
    }
}
