use super::{
    ArrayRead, ArrayReadWrite, ArrayType, DataRead, DataReadWrite, DataType, ReadOnly, ReadWrite,
};
use std::ffi::{CString, NulError};
use std::marker::PhantomData;
use std::os::raw::c_void;
use std::ptr;
use xplm_sys::*;

/// A dataref created by X-Plane or another plugin
///
/// T is the data type stored in the dataref.
///
/// A is the access level (`ReadOnly` or `ReadWrite`)
pub struct DataRef<T: ?Sized, A = ReadOnly> {
    /// The dataref handle
    id: XPLMDataRef,
    /// Type phantom data
    type_phantom: PhantomData<T>,
    /// Data access phantom data
    access_phantom: PhantomData<A>,
}

impl<T: DataType + ?Sized> DataRef<T, ReadOnly> {
    /// Finds a readable dataref by its name
    ///
    /// Returns an error if the dataref does not exist or has the wrong type
    pub fn find(name: &str) -> Result<Self, FindError> {
        let name_c = CString::new(name)?;
        let expected_type = T::sim_type();

        let dataref = unsafe { XPLMFindDataRef(name_c.as_ptr()) };
        if dataref.is_null() {
            return Err(FindError::NotFound);
        }

        let actual_type = unsafe { XPLMGetDataRefTypes(dataref) };
        if actual_type & expected_type != 0 {
            Ok(DataRef {
                id: dataref,
                type_phantom: PhantomData,
                access_phantom: PhantomData,
            })
        } else {
            Err(FindError::WrongType)
        }
    }

    /// Makes this dataref writable
    ///
    /// Returns an error if the dataref cannot be written.
    pub fn writeable(self) -> Result<DataRef<T, ReadWrite>, FindError> {
        let writable = unsafe { XPLMCanWriteDataRef(self.id) == 1 };
        if writable {
            Ok(DataRef {
                id: self.id,
                type_phantom: PhantomData,
                access_phantom: PhantomData,
            })
        } else {
            Err(FindError::NotWritable)
        }
    }
}

/// A dataref that first validates all input and output data before passing it on.
/// This can be used to avoid attempting to write junk into the dataref system, or
/// consuming junk written by somebody else.
///
/// This works the same as a normal DataRef struct, except the second generic
/// argument must be a struct which implements the `DataValidator` trait. See
/// `crate::data::validator` for a list of ready-to-use data validators.
pub struct ValidatedDataRef<T, V, A = ReadOnly>
where
    T: DataType + ?Sized,
    V: super::validator::DataValidator<T::Validation>,
{
    dr: DataRef<T, A>,
    validator: PhantomData<V>,
}

impl<T, V> ValidatedDataRef<T, V, ReadOnly>
where
    T: DataType + ?Sized,
    V: super::validator::DataValidator<T::Validation>,
{
    pub fn find<S: AsRef<str>>(name: S) -> Result<Self, FindError> {
        Ok(Self {
            dr: DataRef::find(name.as_ref())?,
            validator: PhantomData,
        })
    }
    /// Makes this dataref writable
    ///
    /// Returns an error if the dataref cannot be written.
    pub fn writeable(self) -> Result<ValidatedDataRef<T, V, ReadWrite>, FindError> {
        Ok(ValidatedDataRef {
            dr: self.dr.writeable()?,
            validator: PhantomData,
        })
    }
}

pub trait ValidatedDataRead<T, V>
where
    T: DataType,
    V: super::validator::DataValidator<T::Validation>,
{
    fn get(&self) -> Result<T, V::Error>;
}

pub trait ValidatedDataReadWrite<T, V>
where
    T: DataType,
    V: super::validator::DataValidator<T::Validation>,
    Self: ValidatedDataRead<T, V>,
{
    fn set(&mut self, value: T) -> Result<(), V::Error>;
}

#[allow(clippy::len_without_is_empty)]
pub trait ValidatedArrayRead<T, V>
where
    T: ArrayType + ?Sized,
    V: super::validator::DataValidator<T::Validation>,
{
    fn get(&self, dest: &mut [T::Element]) -> Result<usize, V::Error>;
    fn len(&self) -> usize;
}

pub trait ValidatedArrayReadWrite<T, V>
where
    Self: ValidatedArrayRead<T, V>,
    T: ArrayType + ?Sized,
    V: super::validator::DataValidator<T::Validation>,
{
    fn set(&mut self, values: &[T::Element]) -> Result<(), V::Error>;
}

pub struct TypedDataRef<X, R, A = ReadOnly>
where
    X: DataType + ?Sized,
{
    dr: DataRef<X, A>,
    rust_type: PhantomData<R>,
}

impl<X, R> TypedDataRef<X, R, ReadOnly>
where
    X: DataType + ?Sized,
    R: TryFrom<X::Storage>,
{
    pub fn find<S: AsRef<str>>(name: S) -> Result<Self, FindError> {
        Ok(Self {
            dr: DataRef::find(name.as_ref())?,
            rust_type: PhantomData,
        })
    }
}

impl<X, R> TypedDataRef<X, R, ReadOnly>
where
    X: DataType + ?Sized,
    X::Storage: From<R>,
{
    /// Makes this dataref writable
    ///
    /// Returns an error if the dataref cannot be written.
    pub fn writeable(self) -> Result<TypedDataRef<X, R, ReadWrite>, FindError> {
        Ok(TypedDataRef {
            dr: self.dr.writeable()?,
            rust_type: PhantomData,
        })
    }
}

pub trait TypedDataRead<X, R>
where
    X: DataType,
    R: TryFrom<X>,
{
    fn get(&self) -> Result<R, R::Error>;
}

pub trait TypedArrayRead<X, R>
where
    X: ArrayType + ?Sized,
    R: TryFrom<X::Element>,
{
    fn get(&self) -> Result<Vec<R>, R::Error>;
}

pub trait TypedDataReadWrite<X, R>
where
    X: DataType + From<R>,
{
    fn set(&mut self, value: R);
}

pub trait TypedArrayReadWrite<X, R>
where
    X: ArrayType + ?Sized,
    X::Element: From<R>,
{
    fn set(&mut self, values: impl Iterator<Item = R>);
}

/// Creates a DataType implementation, DataRef::get() and DataRef::set() for a type
macro_rules! dataref_type {
    // Basic case
    (
        $(#[$meta:meta])*
        dataref type {
            native $native_type:ty;
            sim $sim_type:ident as $sim_native_type:ty;
            read $read_fn:ident;
            write $write_fn:ident;
        }
    ) => {
        impl<A> DataRead<$native_type> for DataRef<$native_type, A> {
            fn get(&self) -> $native_type {
                unsafe { $read_fn(self.id) as $native_type }
            }
        }
        impl DataReadWrite<$native_type> for DataRef<$native_type, ReadWrite> {
            fn set(&mut self, value: $native_type) {
                unsafe { $write_fn(self.id, value as $sim_native_type) }
            }
        }
        impl<V, A> ValidatedDataRead<$native_type, V> for ValidatedDataRef<$native_type, V, A>
        where
            V: super::validator::DataValidator<$native_type>,
        {
            fn get(&self) -> Result<$native_type, V::Error> {
                let value = self.dr.get();
                V::validate(&value).map(|_| value)
            }
        }
        impl<V> ValidatedDataReadWrite<$native_type, V>
            for ValidatedDataRef<$native_type, V, ReadWrite>
        where
            V: super::validator::DataValidator<$native_type>,
        {
            fn set(&mut self, value: $native_type) -> Result<(), V::Error> {
                V::validate(&value)?;
                self.dr.set(value);
                Ok(())
            }
        }

        impl<A, R> TypedDataRead<$native_type, R> for TypedDataRef<$native_type, R, A>
        where
            R: TryFrom<$native_type>,
        {
            fn get(&self) -> Result<R, R::Error> {
                R::try_from(self.dr.get())
            }
        }
        impl<R> TypedDataReadWrite<$native_type, R> for TypedDataRef<$native_type, R, ReadWrite>
        where
            $native_type: From<R>,
        {
            fn set(&mut self, value: R) {
                self.dr.set(<$native_type>::from(value));
            }
        }
    };
    // Array case
    (
        $(#[$meta:meta])*
        dataref array type {
            native [$native_type:ty];
            sim $sim_type:ident as [$sim_native_type:ty];
            $(#[$read_meta:meta])*
            read $read_fn:ident;
            $(#[$write_meta:meta])*
            write $write_fn:ident;
        }
    ) => {
        impl<A> ArrayRead<[$native_type]> for DataRef<[$native_type], A> {
            #[allow(trivial_casts)]
            fn get(&self, dest: &mut [$native_type]) -> usize {
                let size = array_size(dest.len());
                let copy_count = unsafe {
                    $read_fn(self.id, dest.as_mut_ptr() as *mut $sim_native_type, 0, size)
                };
                copy_count as usize
            }
            fn len(&self) -> usize {
                let size = unsafe { $read_fn(self.id, ptr::null_mut(), 0, 0) };
                size as usize
            }
        }

        impl ArrayReadWrite<[$native_type]> for DataRef<[$native_type], ReadWrite> {
            fn set(&mut self, values: &[$native_type]) {
                let size = array_size(values.len());
                unsafe {
                    // Cast to *mut because the API requires it
                    $write_fn(self.id, values.as_ptr() as *mut $sim_native_type, 0, size);
                }
            }
        }

        impl<V, A> ValidatedArrayRead<[$native_type], V> for ValidatedDataRef<[$native_type], V, A>
        where
            V: super::validator::DataValidator<$native_type>,
        {
            fn get(&self, dest: &mut [$native_type]) -> Result<usize, V::Error> {
                let len = self.dr.get(dest);
                if let Some(e) = dest[..len]
                    .iter()
                    .find_map(|value| V::validate(value).err())
                {
                    // Destroy any bad data to avoid using it
                    dest.iter_mut().for_each(|v| *v = <$native_type>::default());
                    return Err(e);
                }
                Ok(len)
            }
            fn len(&self) -> usize {
                self.dr.len()
            }
        }

        impl<V> ValidatedArrayReadWrite<[$native_type], V>
            for ValidatedDataRef<[$native_type], V, ReadWrite>
        where
            V: super::validator::DataValidator<$native_type>,
        {
            fn set(&mut self, values: &[$native_type]) -> Result<(), V::Error> {
                if let Some(e) = values.iter().find_map(|value| V::validate(value).err()) {
                    return Err(e);
                }
                self.dr.set(values);
                Ok(())
            }
        }

        impl<R> TypedArrayRead<[$native_type], R> for TypedDataRef<[$native_type], R>
        where
            R: TryFrom<$native_type>,
        {
            fn get(&self) -> Result<Vec<R>, R::Error> {
                self.dr
                    .as_vec()
                    .into_iter()
                    .map(|item| R::try_from(item))
                    .collect()
            }
        }

        impl<R> TypedArrayReadWrite<[$native_type], R>
            for TypedDataRef<[$native_type], R, ReadWrite>
        where
            $native_type: From<R>,
        {
            fn set(&mut self, values: impl Iterator<Item = R>) {
                let values = values
                    .map(|value| <$native_type>::from(value))
                    .collect::<Vec<_>>();
                self.dr.set(&values);
            }
        }
    };
}

dataref_type! {
    dataref type {
        native u8;
        sim xplmType_Int as i32;
        read XPLMGetDatai;
        write XPLMSetDatai;
    }
}
dataref_type! {
    dataref type {
        native i8;
        sim xplmType_Int as i32;
        read XPLMGetDatai;
        write XPLMSetDatai;
    }
}
dataref_type! {
    dataref type {
        native u16;
        sim xplmType_Int as i32;
        read XPLMGetDatai;
        write XPLMSetDatai;
    }
}
dataref_type! {
    dataref type {
        native i16;
        sim xplmType_Int as i32;
        read XPLMGetDatai;
        write XPLMSetDatai;
    }
}
dataref_type! {
    dataref type {
        native u32;
        sim xplmType_Int as i32;
        read XPLMGetDatai;
        write XPLMSetDatai;
    }
}
dataref_type! {
    dataref type {
        native i32;
        sim xplmType_Int as i32;
        read XPLMGetDatai;
        write XPLMSetDatai;
    }
}
dataref_type! {
    dataref type {
        native f32;
        sim xplmType_Float as f32;
        read XPLMGetDataf;
        write XPLMSetDataf;
    }
}
dataref_type! {
    dataref type {
        native f64;
        sim xplmType_Double as f64;
        read XPLMGetDatad;
        write XPLMSetDatad;
    }
}

dataref_type! {
    dataref array type {
        native [i32];
        sim xplmType_IntArray as [i32];
        read XPLMGetDatavi;
        write XPLMSetDatavi;
    }
}
dataref_type! {
    dataref array type {
        native [u32];
        sim xplmType_IntArray as [i32];
        read XPLMGetDatavi;
        write XPLMSetDatavi;
    }
}
dataref_type! {
    dataref array type {
        native [f32];
        sim xplmType_FloatArray as [f32];
        read XPLMGetDatavf;
        write XPLMSetDatavf;
    }
}
dataref_type! {
    dataref array type {
        native [u8];
        sim xplmType_Data as [c_void];
        read XPLMGetDatab;
        write XPLMSetDatab;
    }
}
dataref_type! {
    dataref array type {
        native [i8];
        sim xplmType_Data as [c_void];
        read XPLMGetDatab;
        write XPLMSetDatab;
    }
}
impl<A> DataRead<bool> for DataRef<bool, A> {
    fn get(&self) -> bool {
        let int_value = unsafe { XPLMGetDatai(self.id) };
        int_value != 0
    }
}

impl DataReadWrite<bool> for DataRef<bool, ReadWrite> {
    fn set(&mut self, value: bool) {
        let int_value = if value { 1 } else { 0 };
        unsafe { XPLMSetDatai(self.id, int_value) };
    }
}

/// Converts a usize into an i32. Returns i32::MAX if the provided size is too large for an i32
fn array_size(size: usize) -> i32 {
    if size > (i32::MAX as usize) {
        i32::MAX
    } else {
        size as i32
    }
}

/// Errors that can occur when finding DataRefs
#[derive(thiserror::Error, Debug)]
pub enum FindError {
    /// The provided DataRef name contained a null byte
    #[error("Null byte in DataRef name")]
    Null(#[from] NulError),

    /// The DataRef could not be found
    #[error("DataRef not found")]
    NotFound,

    /// The DataRef is not writable
    #[error("DataRef not writable")]
    NotWritable,

    /// The DataRef does not have the correct type
    #[error("Incorrect DataRef type")]
    WrongType,
}

#[cfg(test)]
mod tests {
    /// Checks that the as operator truncates values
    #[test]
    fn test_as_truncate() {
        let x = 0x11223344u32;
        let x8 = x as u8;
        assert_eq!(x8, 0x44u8);
    }
}
