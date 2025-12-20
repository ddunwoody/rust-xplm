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

use crate::data::owned::CreateError;
use crate::data::{owned::OwnedData, ReadOnly};
use crate::data::{Access, ArrayType, DataType};

use std::marker::PhantomData;

/// An owned dataref, which includes type, conversion and validation information. This
/// lets you read/write datarefs using non-primitive Rust types and have the system
/// automatically convert between the Rust and XPLM data types as necessary. On read-back
/// validation is performed, to make sure the conversion is successful.
///
/// This type takes the following 4 generic type arguments:
///
/// - `X` - the X-Plane native data type. This must be one of:
///   - `bool`
///   - `i8` or `u8`
///   - `i16` or `u16`
///   - `i32` or `u32`
///   - `f32`
///   - `f64`
///   - `[bool]`
///   - `[i8]` or `[u8]`
///   - `[i32]` or `[u32]`
///   - `[f32]`
/// - `R` - the Rust type, which we will convert into/out of when writing the dataref.
/// - `C` - the Conversion type. This type must implement `xplm::data::typed::InputDataConversion`
///   to enable reading the dataref, and `xplm::data::typed::OutputDataConversion` to enable
///   writing to it.
/// - `A` - an optional access argument. Must be either `ReadOnly` (the default), or `ReadWrite`.
///   Please note that owned datarefs are always writable by *us*, since we own the data.
///   This type argument instead denotes whether the dataref should be writable to other
///   plugins.
///
/// # Example Enum Owned DataRef Read & Write
/// ```no_run
/// use xplm::data::ReadWrite;
/// use xplm::data::typed::owned::TypedOwnedData;
/// use xplm::data::typed::{TypedDataRead, TypedDataReadWrite};
/// use xplm::data::typed::{InputUnitConversion, OutputUnitConversion};
/// // Create the dataref and populate it with the default value of MyEnum. This can fail
/// // if the dataref already exists.
/// let mut my_dr = TypedOwnedData::<i32, MyEnum, MyEnum>::create("my/enumdata").unwrap();
/// #[derive(Copy, Clone, Debug, Default)]
/// enum MyEnum {
///     ValueA = 0,
///     #[default]
///     ValueB = 1,
///     ValueC = 2,
/// }
/// // Implement the necessary traits for MyEnum to enable the typed dataref
/// // to perform the input/output conversion and validation.
/// #[derive(Debug)]
/// struct InvalidMyEnum(i32);
/// impl InputUnitConversion<i32, MyEnum> for MyEnum {
///     type Error = InvalidMyEnum;
///     fn try_conv_in(value: i32) -> Result<MyEnum, Self::Error> {
///         match value {
///             x if x == MyEnum::ValueA as i32 => Ok(MyEnum::ValueA),
///             x if x == MyEnum::ValueB as i32 => Ok(MyEnum::ValueB),
///             x if x == MyEnum::ValueC as i32 => Ok(MyEnum::ValueC),
///             _ => Err(InvalidMyEnum(value)),
///         }
///     }
/// }
/// impl OutputUnitConversion<MyEnum, i32> for MyEnum {
///     fn conv_out(value: &MyEnum) -> i32 {
///         *value as i32
///     }
/// }
/// // We can now read and write `MyEnum' enums directly to/from the dataref:
/// match my_dr.get() {
///     Ok(my_enum) => assert!(matches!(my_enum, MyEnum::ValueB)),
///     Err(e) => println!("got invalid MyEnum value: {}", e.0),
/// }
/// my_dr.set(MyEnum::ValueC);
/// assert!(!matches!(my_dr.get().unwrap(), MyEnum::ValueA));
/// assert!(!matches!(my_dr.get().unwrap(), MyEnum::ValueB));
/// assert!(matches!(my_dr.get().unwrap(), MyEnum::ValueC));
/// ```
pub type TypedOwnedData<X, R, C, A = ReadOnly> = super::TypedData<X, R, C, OwnedData<X, A>>;

impl<X, R, C, A> TypedOwnedData<X, R, C, A>
where
    X: DataType,
    A: Access,
{
    /// Creates a new dataref with the provided name containing the default value of `R`.
    /// This function is invoked for single-element (non-array) owned datarefs. The Rust
    /// type `R` associated with this dataref must implement the `Default` trait.
    pub fn create(name: &str) -> Result<Self, CreateError>
    where
        R: Default,
        C: super::OutputUnitConversion<R, X>,
    {
        let v = C::conv_out(&R::default());
        Ok(Self {
            dr: OwnedData::create_with_value(name, &v)?,
            data: PhantomData,
            rust_type: PhantomData,
            conv: PhantomData,
        })
    }
    /// Creates a new dataref with the provided name and value. This function is invoked
    /// for single-element (non-array) owned datarefs.
    pub fn create_with_value(name: &str, value: R) -> Result<Self, CreateError>
    where
        C: super::OutputUnitConversion<R, X>,
    {
        Ok(Self {
            dr: OwnedData::create_with_value(name, &C::conv_out(&value))?,
            data: PhantomData,
            rust_type: PhantomData,
            conv: PhantomData,
        })
    }
}

impl<X, R, C, A> TypedOwnedData<[X], R, C, A>
where
    [X]: ArrayType,
    A: Access,
{
    /// Creates a new dataref with the provided name containing an array of `R`, of
    /// length `len`. The Rust type `R` associated with this dataref must implement
    /// the `Default` and `Clone` traits.
    pub fn create(name: &str, len: usize) -> Result<Self, CreateError>
    where
        X: Clone,
        R: Default,
        C: super::OutputUnitConversion<R, X>,
    {
        let v = C::conv_out(&R::default());
        let values = vec![v; len];
        Ok(Self {
            dr: OwnedData::create_with_value(name, &values[..])?,
            data: PhantomData,
            rust_type: PhantomData,
            conv: PhantomData,
        })
    }
    /// Creates a new dataref with the provided name and values.
    pub fn create_with_value(name: &str, values: &[R]) -> Result<Self, CreateError>
    where
        C: super::OutputUnitConversion<R, X>,
    {
        let values = values
            .iter()
            .map(|value| C::conv_out(value))
            .collect::<Vec<_>>();
        Ok(Self {
            dr: OwnedData::create_with_value(name, &values[..])?,
            data: PhantomData,
            rust_type: PhantomData,
            conv: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::data::{
        typed::{
            owned::TypedOwnedData, InputUnitConversion, OutputUnitConversion, TypedArrayRead,
            TypedArrayReadWrite, TypedDataRead, TypedDataReadWrite,
        },
        ReadWrite,
    };

    #[test]
    fn test_typed_owned_data() {
        let _dr_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();

        #[derive(Copy, Clone, Default, derive_more::TryFrom, Debug, PartialEq, Eq)]
        #[try_from(repr)]
        #[repr(u32)]
        enum ValidValues {
            #[default]
            A,
            B,
            C,
        }
        struct ValidValuesConv {}
        impl InputUnitConversion<u32, ValidValues> for ValidValuesConv {
            type Error = derive_more::TryFromReprError<u32>;
            fn try_conv_in(value: u32) -> Result<ValidValues, Self::Error> {
                ValidValues::try_from(value)
            }
        }
        impl OutputUnitConversion<ValidValues, u32> for ValidValuesConv {
            fn conv_out(value: &ValidValues) -> u32 {
                *value as _
            }
        }
        let mut dr_ro =
            TypedOwnedData::<u32, ValidValues, ValidValuesConv>::create("test/new/u32_1").unwrap();
        dr_ro.set(ValidValues::B);
        assert_eq!(dr_ro.get().unwrap(), ValidValues::B);

        let mut dr_rw = TypedOwnedData::<u32, ValidValues, ValidValuesConv, ReadWrite>::create(
            "test/new/u32_2",
        )
        .unwrap();
        dr_rw.set(ValidValues::C);
        assert_eq!(dr_rw.get().unwrap(), ValidValues::C);

        let mut dr_array =
            TypedOwnedData::<[u32], ValidValues, ValidValuesConv>::create("test/new/u32_3", 2)
                .unwrap();
        dr_array.set_subdata(&[ValidValues::C], 1);
        assert_eq!(dr_array.get().unwrap(), [ValidValues::A, ValidValues::C]);
    }
}
