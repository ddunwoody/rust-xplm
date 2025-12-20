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

use std::marker::PhantomData;

use crate::data::{
    borrowed::{DataRef, FindError},
    DataType, ReadOnly, ReadWrite,
};

/// A borrowed dataref, which includes type, conversion and validation information. This
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
///
/// # Example Enum DataRef Read & Write
/// ```no_run
/// use xplm::data::ReadWrite;
/// use xplm::data::typed::borrowed::TypedDataRef;
/// use xplm::data::typed::{TypedDataRead, TypedDataReadWrite};
/// use xplm::data::typed::{InputUnitConversion, OutputUnitConversion};
///
/// // Look up the dataref first
/// let mut dg_source_dr: TypedDataRef<i32, DGSource, DGSource, ReadWrite> =
///     // Looks up a read-only version of the dataref.
///     TypedDataRef::find("sim/aircraft/autopilot/dg_source")
///         // This can fail if the dataref doesn't exist.
///         .unwrap()
///         // Attempts to convert the dataref to enable writing.
///         .writeable()
///         // This can fail if the dataref isn't writable.
///         .unwrap();
/// // DGSource is a plain enum, conforming to the enum description in X-Plane's DataRefs.txt:
/// // 10 = AHRS, 11 = elec gyro (HSI or DG), 12 = vacuum gyro (DG)
/// #[derive(Copy, Clone, Debug)]
/// enum DGSource {
///     AHRS = 10,
///     ElecGyro = 11,
///     VacuumGyro = 12,
/// }
/// // Implement the necessary traits for DGSource to enable the typed dataref
/// // to perform the input/output conversion and validation.
/// #[derive(Debug)]
/// struct InvalidDGSource(i32);
/// impl InputUnitConversion<i32, DGSource> for DGSource {
///     type Error = InvalidDGSource;
///     fn try_conv_in(value: i32) -> Result<DGSource, Self::Error> {
///         match value {
///             x if x == DGSource::AHRS as i32 => Ok(DGSource::AHRS),
///             x if x == DGSource::ElecGyro as i32 => Ok(DGSource::ElecGyro),
///             x if x == DGSource::VacuumGyro as i32 => Ok(DGSource::VacuumGyro),
///             _ => Err(InvalidDGSource(value)),
///         }
///     }
/// }
/// impl OutputUnitConversion<DGSource, i32> for DGSource {
///     fn conv_out(value: &DGSource) -> i32 {
///         *value as i32
///     }
/// }
/// // We can now read and write `DGSource' enums directly to/from the dataref:
/// match dg_source_dr.get() {
///     Ok(dg_source) => println!("got DG source: {dg_source:?}"),
///     Err(e) => println!("got invalid DG source enum value: {}", e.0),
/// }
/// dg_source_dr.set(DGSource::VacuumGyro);
/// assert!(!matches!(dg_source_dr.get().unwrap(), DGSource::AHRS));
/// assert!(!matches!(dg_source_dr.get().unwrap(), DGSource::ElecGyro));
/// assert!(matches!(dg_source_dr.get().unwrap(), DGSource::VacuumGyro));
/// ```
/// # Example with an array dataref
/// ```no_run
/// use xplm::data::typed::borrowed::TypedDataRef;
/// use xplm::data::typed::{TypedArrayRead, InputUnitConversion};
///
/// // Looks up a read-only version of the dataref.
/// // This can fail if the dataref doesn't exist.
/// let mut propmode_dr =
///     TypedDataRef::<[i32], PropMode, PropMode>::find("sim/flightmodel/engine/ENGN_propmode")
///         .unwrap();
/// #[derive(Copy, Clone, Debug)]
/// enum PropMode {
///     Feather = 0,
///     Normal = 1,
///     Beta = 2,
///     Reverse = 3,
/// }
/// // Implement the necessary traits for DGSource to enable the typed dataref
/// // to perform the input/output conversion and validation.
/// #[derive(Debug)]
/// struct InvalidPropMode {};
/// impl InputUnitConversion<i32, PropMode> for PropMode {
///     type Error = InvalidPropMode;
///     fn try_conv_in(value: i32) -> Result<PropMode, Self::Error> {
///         match value {
///             x if x == PropMode::Feather as i32 => Ok(PropMode::Feather),
///             x if x == PropMode::Normal as i32 => Ok(PropMode::Normal),
///             x if x == PropMode::Beta as i32 => Ok(PropMode::Beta),
///             x if x == PropMode::Reverse as i32 => Ok(PropMode::Reverse),
///             _ => Err(InvalidPropMode {}),
///         }
///     }
/// }
/// // We can now read `PropMode' enums directly from the dataref. Let's assume airplane
/// // has two engines, so we'll only fetch the propmode for the engines we have.
/// match propmode_dr.get_subdata(0..2) {
///     Ok(propmode_vec) => {
///         println!(
///             "propmode left engine: {:?}  right engine: {:?}",
///             propmode_vec[0],
///             propmode_vec[1],
///         );
///     },
///     Err(_) => println!("got invalid PropMode enum value"),
/// }
/// ```
pub type TypedDataRef<X, R, C, A = ReadOnly> = super::TypedData<X, R, C, DataRef<X, A>>;

impl<X, R, C> TypedDataRef<X, R, C, ReadOnly>
where
    X: DataType + ?Sized,
{
    /// Looks up the dataref and attempts to obtain a read-only reference to it.
    ///
    /// Returns an error if the dataref doesn't exist.
    pub fn find<S: AsRef<str>>(name: S) -> Result<Self, FindError> {
        Ok(Self {
            dr: DataRef::find(name.as_ref())?,
            data: PhantomData,
            rust_type: PhantomData,
            conv: PhantomData,
        })
    }
    /// Makes this dataref writable
    ///
    /// Returns an error if the dataref cannot be written.
    pub fn writeable(self) -> Result<TypedDataRef<X, R, C, ReadWrite>, FindError> {
        Ok(TypedDataRef {
            dr: self.dr.writeable()?,
            data: PhantomData,
            rust_type: PhantomData,
            conv: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::data::typed::{
        borrowed::TypedDataRef, InputUnitConversion, OutputUnitConversion, TypedArrayRead,
        TypedArrayReadWrite, TypedDataRead, TypedDataReadWrite,
    };

    #[test]
    fn test_typed_dataref() {
        let _dr_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();

        #[derive(Copy, Clone, derive_more::TryFrom, PartialEq, Eq, Debug)]
        #[try_from(repr)]
        #[repr(i32)]
        enum ValidValues {
            A,
            B,
            C,
        }
        struct ValidValuesConv {}
        impl InputUnitConversion<i32, ValidValues> for ValidValuesConv {
            type Error = derive_more::TryFromReprError<i32>;
            fn try_conv_in(value: i32) -> Result<ValidValues, Self::Error> {
                ValidValues::try_from(value)
            }
        }
        impl OutputUnitConversion<ValidValues, i32> for ValidValuesConv {
            fn conv_out(value: &ValidValues) -> i32 {
                *value as _
            }
        }

        let mut dr = TypedDataRef::<i32, ValidValues, ValidValuesConv>::find("test/i32")
            .unwrap()
            .writeable()
            .unwrap();
        let en = ValidValues::C;
        dr.set(en);
        assert_ne!(dr.get().unwrap(), ValidValues::A);
        assert_ne!(dr.get().unwrap(), ValidValues::B);
        assert_eq!(dr.get().unwrap(), ValidValues::C);

        let mut array_dr =
            TypedDataRef::<[i32], ValidValues, ValidValuesConv>::find("test/i32array")
                .unwrap()
                .writeable()
                .unwrap();
        let en = ValidValues::C;
        array_dr.set(&[en]);
        let en_out = array_dr.get_subdata(0..1).unwrap();
        assert_eq!(en_out, vec![ValidValues::C]);
    }
}
