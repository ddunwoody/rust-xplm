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

#[macro_export]
macro_rules! uom_typed_dataref {
    (
        $modname:ident,
        uom: $(::)?$($uom_type:ident)::+,
        phys_unit: $unit_name:ty,
        range: ($minval:literal..$maxval:literal),
    ) => {
        pub mod $modname {
            use ::uom::si::SI;
            use $crate::data::typed::{InputUnitConversion, OutputUnitConversion};
            use $crate::data::typed::borrowed::TypedDataRef;
            use $crate::data::typed::owned::TypedOwnedData;
            use $crate::data::DataType;

            #[derive(Copy, Clone, Debug)]
            pub struct Conv {}

            #[derive(Copy, Clone, Debug, Eq, PartialEq)]
            pub struct InvalidValueError {}

            impl<V> InputUnitConversion<V, $($uom_type)::*<SI<V>, V>> for Conv
            where
                V: ::num::Num + ::uom::Conversion<V, T = V> + From<f32> + PartialOrd,
                SI<V>: ::uom::si::Units<V>,
                $unit_name: ::uom::Conversion<V, T = V>,
            {
                type Error = InvalidValueError;
                fn try_conv_in(value: V) -> Result<$($uom_type)::*<SI<V>, V>, Self::Error> {
                    (value > V::from($minval) && value < V::from($maxval))
                        .then(|| $($uom_type)::*::new::<$unit_name>(value))
                        .ok_or(InvalidValueError {})
                }
            }
            impl<V> OutputUnitConversion<$($uom_type)::*<SI<V>, V>, V> for Conv
            where
                V: ::num::Num + ::uom::Conversion<V, T = V>,
                SI<V>: ::uom::si::Units<V>,
                $unit_name: ::uom::Conversion<V, T = V>,
            {
                fn conv_out(value: $($uom_type)::*<SI<V>, V>) -> V {
                    value.get::<$unit_name>()
                }
            }
            #[allow(dead_code)]
            pub type DataRef<V, A = $crate::data::ReadOnly> =
                TypedDataRef<
                    V,
                    $($uom_type)::*<SI<<V as DataType>::Validation>, <V as DataType>::Validation>,
                    Conv,
                    A,
                >;
            #[allow(dead_code)]
            pub type OwnedData<V, A = $crate::data::ReadOnly> =
                TypedOwnedData<
                    V,
                    $($uom_type)::*<SI<<V as DataType>::Validation>, <V as DataType>::Validation>,
                    Conv,
                    A,
                >;
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::data::typed::{
        TypedArrayRead, TypedArrayReadWrite, TypedDataRead, TypedDataReadWrite,
    };
    use uom::si::{
        f32::ThermodynamicTemperature,
        thermodynamic_temperature::{degree_celsius, kelvin},
    };

    uom_typed_dataref!(
        temperature_celsius,
        uom: uom::si::thermodynamic_temperature::ThermodynamicTemperature,
        phys_unit: uom::si::thermodynamic_temperature::degree_celsius,
        range: (-273.15_f32..5000_f32),
    );

    #[test]
    fn test_temperature_dataref() {
        let _dr_lock = crate::test_stubs::DATAREF_SYS_LOCK.lock();

        let mut dr = temperature_celsius::DataRef::<f32>::find("test/f32")
            .unwrap()
            .writeable()
            .unwrap();
        dr.set(ThermodynamicTemperature::new::<degree_celsius>(5.0));
        assert_eq!(
            dr.get().unwrap(),
            ThermodynamicTemperature::new::<kelvin>(278.15)
        );
        // Try to shove something bad into the dataref to check input range checking
        dr.set(ThermodynamicTemperature::new::<kelvin>(0.0));
        assert_eq!(
            dr.get().unwrap_err(),
            temperature_celsius::InvalidValueError {}
        );

        let mut dr_owned =
            temperature_celsius::OwnedData::<f32>::create("test/owned/temp_cel").unwrap();
        dr_owned.set(ThermodynamicTemperature::new::<degree_celsius>(100.0));
        assert_eq!(
            dr_owned.get().unwrap(),
            ThermodynamicTemperature::new::<kelvin>(373.15)
        );

        let mut dr_owned_array =
            temperature_celsius::OwnedData::<[f32]>::create("test/owned/temp_cel_array", 2)
                .unwrap();
        dr_owned_array.set([ThermodynamicTemperature::new::<degree_celsius>(0.0); 2].into_iter());
        assert_eq!(
            dr_owned_array.get().unwrap(),
            [ThermodynamicTemperature::new::<kelvin>(273.15); 2],
        );
    }
}
