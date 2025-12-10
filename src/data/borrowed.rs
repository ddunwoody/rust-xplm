use super::{
    ArrayRead, ArrayReadWrite, ArrayType, DataRead, DataReadWrite, DataType, ReadOnly, ReadWrite,
};
use std::ffi::{CString, NulError};
use std::marker::PhantomData;
use std::num::FpCategory;
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
            V: DataValidator<$native_type>,
        {
            fn get(&self) -> Result<$native_type, V::Error> {
                let value = self.dr.get();
                V::validate(&value).map(|_| value)
            }
        }
        impl<V> ValidatedDataReadWrite<$native_type, V>
            for ValidatedDataRef<$native_type, V, ReadWrite>
        where
            V: DataValidator<$native_type>,
        {
            fn set(&mut self, value: $native_type) -> Result<(), V::Error> {
                V::validate(&value)?;
                self.dr.set(value);
                Ok(())
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

        impl<V: DataValidator<$native_type>, A> ValidatedArrayRead<[$native_type], V>
            for ValidatedDataRef<[$native_type], V, A>
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

        impl<V: DataValidator<$native_type>> ValidatedArrayReadWrite<[$native_type], V>
            for ValidatedDataRef<[$native_type], V, ReadWrite>
        {
            fn set(&mut self, values: &[$native_type]) -> Result<(), V::Error> {
                if let Some(e) = values.iter().find_map(|value| V::validate(value).err()) {
                    return Err(e);
                }
                self.dr.set(values);
                Ok(())
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

pub struct ValidatedDataRef<T, V, A = ReadOnly>
where
    T: DataType + ?Sized,
    V: DataValidator<T::Validation>,
{
    dr: DataRef<T, A>,
    validator: PhantomData<V>,
}

impl<T, V> ValidatedDataRef<T, V, ReadOnly>
where
    T: DataType + ?Sized,
    V: DataValidator<T::Validation>,
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
    V: DataValidator<T::Validation>,
{
    fn get(&self) -> Result<T, V::Error>;
}

pub trait ValidatedDataReadWrite<T, V>
where
    T: DataType,
    V: DataValidator<T::Validation>,
    Self: ValidatedDataRead<T, V>,
{
    fn set(&mut self, value: T) -> Result<(), V::Error>;
}

#[allow(clippy::len_without_is_empty)]
pub trait ValidatedArrayRead<T, V>
where
    T: ArrayType + ?Sized,
    V: DataValidator<T::Validation>,
{
    fn get(&self, dest: &mut [T::Element]) -> Result<usize, V::Error>;
    fn len(&self) -> usize;
}

pub trait ValidatedArrayReadWrite<T, V>
where
    Self: ValidatedArrayRead<T, V>,
    T: ArrayType + ?Sized,
    V: DataValidator<T::Validation>,
{
    fn set(&mut self, values: &[T::Element]) -> Result<(), V::Error>;
}

pub trait DataValidator<T: ?Sized> {
    type Error;
    fn validate(_data: &T) -> Result<(), Self::Error>;
}

pub enum FloatValidationError {
    NotNormal(FpCategory),
    Negative,
}

pub struct NormalFloat<T: num::Float> {
    phantom: PhantomData<T>,
}

impl<T: num::Float> DataValidator<T> for NormalFloat<T> {
    type Error = FloatValidationError;
    fn validate(data: &T) -> Result<(), Self::Error> {
        match data.classify() {
            FpCategory::Normal => Ok(()),
            cat => Err(FloatValidationError::NotNormal(cat)),
        }
    }
}

pub struct NonNegativeFloat<T: num::Float> {
    phantom: PhantomData<T>,
}

impl<T: num::Float> DataValidator<T> for NonNegativeFloat<T> {
    type Error = FloatValidationError;
    fn validate(data: &T) -> Result<(), Self::Error> {
        match data.classify() {
            FpCategory::Normal => (*data >= T::zero())
                .then_some(())
                .ok_or(FloatValidationError::Negative),
            cat => Err(FloatValidationError::NotNormal(cat)),
        }
    }
}

pub struct PositiveFloat<T: num::Float> {
    phantom: PhantomData<T>,
}

impl<T: num::Float> DataValidator<T> for PositiveFloat<T> {
    type Error = FloatValidationError;
    fn validate(data: &T) -> Result<(), Self::Error> {
        match data.classify() {
            FpCategory::Normal => (*data > T::zero())
                .then_some(())
                .ok_or(FloatValidationError::Negative),
            cat => Err(FloatValidationError::NotNormal(cat)),
        }
    }
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
