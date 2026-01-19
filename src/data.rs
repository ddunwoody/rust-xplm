use crate::ffi::StringBuffer;
use std::ffi::{c_double, c_float, c_int, c_void, CString, NulError};
use std::string::FromUtf8Error;
use xplm_sys::*;

/// Datarefs created by X-Plane or other plugins
pub mod borrowed;
/// Datarefs created by this plugin, but whose contents are generated dynamically.
pub mod dynamic;
/// Datarefs created by this plugin
pub mod owned;

/// Datarefs which support direct setting & getting via custom Rusty types.
/// Use this module to provide maximum input/output validation and validity
/// checking. This is useful when dealing with datarefs which represent
/// physical quantities, like temperatures, lengths or pressures.
pub mod typed;

/// Datarefs which allow creating custom range/value validation rules, without
/// the need to implement full conversion into/out of your own Rust types.
/// This is useful when dealing with datarefs which represent abstract ratios
/// or numerical quantities, not directly correlated to physical units (e.g.
/// joystick axis ranges, simulation rates, etc.).
pub mod validated;

/// Marks a dataref as readable
pub enum ReadOnly {}

/// Marks a dataref as writeable
pub enum ReadWrite {}

/// Marker for data access types
pub trait Access {
    /// Returns true if this access allows the dataref to be written
    fn writeable() -> bool;
}

impl Access for ReadOnly {
    fn writeable() -> bool {
        false
    }
}

impl Access for ReadWrite {
    fn writeable() -> bool {
        true
    }
}

/// Trait for data accessors that can be read
pub trait DataRead<T> {
    /// Reads a value
    fn get(&self) -> T;
}

/// Trait for writable data accessors
pub trait DataReadWrite<T>: DataRead<T> {
    /// Writes a value
    fn set(&mut self, value: T);
}

/// Trait for readable array data accessors
#[allow(clippy::len_without_is_empty)]
pub trait ArrayRead<T: ArrayType + ?Sized> {
    /// Reads values
    ///
    /// Values are stored in the provided slice. If the dataref is larger than the provided slice,
    /// values beyond the bounds of the slice are ignored.
    ///
    /// If the dataref is smaller than the provided slice, the extra values in the slice will not
    /// be modified.
    ///
    /// The maximum number of values in an array dataref is i32::MAX.
    ///
    /// This function returns the number of values that were read.
    #[must_use]
    fn get(&self, dest: &mut [T::Element]) -> usize {
        self.get_subdata(dest, 0)
    }
    #[must_use]
    fn get_subdata(&self, dest: &mut [T::Element], start_offset: usize) -> usize;

    /// Returns the length of the data array
    fn len(&self) -> usize;

    /// Returns all values in this accessor as a Vec
    fn as_vec(&self) -> Vec<T::Element>
    where
        T::Element: Default + Clone,
    {
        let mut values = vec![T::Element::default(); self.len()];
        let act_len = self.get(&mut values);
        values.truncate(act_len);
        values
    }

    /// Same as `as_vec()`, but allows you to specify a range of subdata to
    /// request from the accessor. The returned vector might be shorter than
    /// the request if the specified range exceeds the length of the dataref.
    fn as_vec_subdata(&self, range: impl std::ops::RangeBounds<usize>) -> Vec<T::Element>
    where
        T::Element: Default + Clone,
    {
        let start = match range.start_bound() {
            std::ops::Bound::Included(start) => *start,
            std::ops::Bound::Excluded(start) => *start + 1,
            std::ops::Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            std::ops::Bound::Included(end) => *end + 1,
            std::ops::Bound::Excluded(end) => *end,
            std::ops::Bound::Unbounded => self.len(),
        };
        let req_len = end - start;
        let mut values = vec![T::Element::default(); req_len];
        let act_len = self.get_subdata(&mut values, start);
        values.truncate(act_len);
        values
    }
}

/// Trait for array accessors that can be read and written
pub trait ArrayReadWrite<T: ArrayType + ?Sized>: ArrayRead<T> {
    /// Writes values
    ///
    /// Values are taken from the provided slice. If the dataref is larger than the provided slice,
    /// values beyond the bounds of the slice are not changed.
    ///
    /// If the dataref is smaller than the provided slice, the values beyond the dataref bounds
    /// will be ignored.
    fn set(&mut self, values: &[T::Element]) {
        self.set_subdata(values, 0);
    }

    fn set_subdata(&mut self, values: &[T::Element], offset: usize);
}

/// Trait for data accessors that can be read as strings
pub trait StringRead {
    /// Reads the value of this dataref and appends it to the provided string
    ///
    /// Returns an error if the dataref is not valid UTF-8.
    ///
    /// If the provided string is not empty, the value of the dataref will be appended to it.
    fn get_to_string(&self, out: &mut String) -> Result<(), FromUtf8Error>;

    /// Reads the value of this dataref as a string and returns it
    fn get_as_string(&self) -> Result<String, FromUtf8Error>;
}

/// Trait for data accessors that can be written as strings
pub trait StringReadWrite: StringRead {
    /// Sets the value of this dataref from a string
    ///
    /// Returns an error if the string contains a null byte
    fn set_as_string(&mut self, value: &str) -> Result<(), NulError>;
}

impl<T> StringRead for T
where
    T: ArrayRead<[u8]>,
{
    fn get_to_string(&self, out: &mut String) -> Result<(), FromUtf8Error> {
        let mut buffer = StringBuffer::new(self.len());
        let act_len = self.get(buffer.as_bytes_mut());
        buffer.truncate(act_len);
        let value_string = buffer.into_string()?;
        out.push_str(&value_string);
        Ok(())
    }
    fn get_as_string(&self) -> Result<String, FromUtf8Error> {
        let mut buffer = StringBuffer::new(self.len());
        let act_len = self.get(buffer.as_bytes_mut());
        buffer.truncate(act_len);
        buffer.into_string()
    }
}

impl<T> StringReadWrite for T
where
    T: ArrayReadWrite<[u8]>,
{
    fn set_as_string(&mut self, value: &str) -> Result<(), NulError> {
        let name_c = CString::new(value)?;
        self.set(name_c.as_bytes_with_nul());
        Ok(())
    }
}

/// Marker for types that can be used with datarefs
pub trait DataType: OwnedDataType {
    /// The type that should be used to store data of this type
    /// For basic types, this is usually Self. For [T] types, this is Vec<T>.
    #[doc(hidden)]
    type Storage: Sized;
    /// The type used for validation.
    #[doc(hidden)]
    type Validation: Sized;
    /// Returns the X-Plane data type corresponding with this type
    #[doc(hidden)]
    fn sim_type() -> XPLMDataTypeID;
    /// Creates an instance of a storage type from an instance of self
    #[doc(hidden)]
    fn to_storage(&self) -> Self::Storage;
}

/// Marker for types that are arrays
pub trait ArrayType: DataType {
    /// The type of the array element
    type Element;
}

macro_rules! impl_type {
    ($native_type:ty as $sim_type:ident) => {
        impl DataType for $native_type {
            type Storage = Self;
            type Validation = $native_type;
            fn sim_type() -> XPLMDataTypeID {
                $sim_type as XPLMDataTypeID
            }
            fn to_storage(&self) -> Self::Storage {
                self.clone()
            }
        }
    };
    ([$native_type:ty]: array as $sim_type:ident) => {
        impl DataType for [$native_type] {
            type Storage = Vec<$native_type>;
            type Validation = $native_type;
            fn sim_type() -> XPLMDataTypeID {
                $sim_type as XPLMDataTypeID
            }
            fn to_storage(&self) -> Self::Storage {
                self.to_vec()
            }
        }
        impl ArrayType for [$native_type] {
            type Element = $native_type;
        }
    };
}

impl_type!(bool as xplmType_Int);
impl_type!(u8 as xplmType_Int);
impl_type!(i8 as xplmType_Int);
impl_type!(u16 as xplmType_Int);
impl_type!(i16 as xplmType_Int);
impl_type!(u32 as xplmType_Int);
impl_type!(i32 as xplmType_Int);
impl_type!(f32 as xplmType_Float);
impl_type!(f64 as xplmType_Double);
impl_type!([bool]: array as xplmType_IntArray);
impl_type!([i32]: array as xplmType_IntArray);
impl_type!([u32]: array as xplmType_IntArray);
impl_type!([f32]: array as xplmType_FloatArray);
impl_type!([u8]: array as xplmType_Data);
impl_type!([i8]: array as xplmType_Data);

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
    u32,
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
    u32,
    c_int,
    int_array_read,
    XPLMGetDatavi_f,
    int_array_write,
    XPLMSetDatavi_f,
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
impl_array_read_write!(
    i8,
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
