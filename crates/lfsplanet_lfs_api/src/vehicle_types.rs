//! Categorical codes used by the LFS Vehicle Mods API.
//!
//! Explicit discriminants match LFS codes; public JSON uses named variants.
//! Measurements, counts and revisions are numbers rather than categories.

macro_rules! vehicle_code {
    ($(#[$meta:meta])* pub enum $name:ident { $($variant:ident = $code:tt),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
        #[serde(rename_all = "snake_case")]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #[repr(u8)]
        pub enum $name { $($variant = $code),+ }

        impl TryFrom<u8> for $name {
            type Error = String;
            fn try_from(value: u8) -> Result<Self, Self::Error> {
                match value {
                    $($code => Ok(Self::$variant),)+
                    _ => Err(format!("invalid {} code: {value}", stringify!($name))),
                }
            }
        }
        impl From<$name> for u8 {
            fn from(value: $name) -> Self { value as Self }
        }
    };
}

vehicle_code! {
    /// Vehicle category. Code 14 is retained from the existing catalogue contract;
    /// its upstream label has not been established.
    #[cfg_attr(feature = "sea-orm", derive(sea_orm::EnumIter, sea_orm::DeriveActiveEnum))]
    #[cfg_attr(feature = "sea-orm", sea_orm(rs_type = "i16", db_type = "SmallInteger"))]
    pub enum VehicleClass {
        Object = 0,
        TouringCar = 1,
        SaloonCar = 2,
        Buggy = 3,
        Formula = 4,
        Gt = 5,
        Kart = 6,
        Bike = 7,
        Van = 8,
        Truck = 9,
        Formula1 = 10,
        FormulaSae = 11,
        Bus = 12,
        Prototype = 13,
        Reserved14 = 14,
    }
}

vehicle_code! {
    /// Combustion engine cylinder layout (independent of cylinder count).
    pub enum EngineLayout {
        Inline = 0,
        Flat = 1,
        V = 2,
    }
}

vehicle_code! {
    /// Driven wheels; `None` is an explicit upstream value, not missing data.
    pub enum Drivetrain {
        None = 0,
        RearWheelDrive = 1,
        FrontWheelDrive = 2,
        AllWheelDrive = 3,
    }
}

vehicle_code! {
    /// Gear selection and clutch mechanism.
    pub enum Transmission {
        None = 0,
        HPattern = 1,
        Motorbike = 2,
        Sequential = 3,
        SequentialWithCut = 4,
        Paddle = 5,
        Electric = 6,
        CentrifugalClutch = 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_json_and_numeric_codes_are_independent() {
        assert_eq!(u8::from(Drivetrain::RearWheelDrive), 1);
        assert_eq!(Drivetrain::try_from(1).unwrap(), Drivetrain::RearWheelDrive);
        assert_eq!(
            serde_json::to_value(Drivetrain::RearWheelDrive).unwrap(),
            "rear_wheel_drive"
        );
        assert_eq!(
            serde_json::from_value::<Transmission>(serde_json::json!("h_pattern")).unwrap(),
            Transmission::HPattern
        );
        assert!(serde_json::from_value::<Drivetrain>(serde_json::json!(1)).is_err());
        assert!(serde_json::from_value::<Drivetrain>(serde_json::json!("unknown")).is_err());
        assert!(Drivetrain::try_from(4).is_err());
        assert!(EngineLayout::try_from(3).is_err());
        assert!(Transmission::try_from(8).is_err());
        assert!(VehicleClass::try_from(15).is_err());
    }

    #[test]
    #[cfg(feature = "openapi")]
    fn schemas_describe_named_wire_values() {
        use utoipa::PartialSchema;
        let schema = serde_json::to_value(Drivetrain::schema()).unwrap();
        assert_eq!(schema["type"], "string");
        assert_eq!(
            schema["enum"],
            serde_json::json!([
                "none",
                "rear_wheel_drive",
                "front_wheel_drive",
                "all_wheel_drive"
            ])
        );
    }

    #[test]
    #[cfg(feature = "sea-orm")]
    fn vehicle_classes_keep_the_existing_smallint_storage() {
        use sea_orm::ActiveEnum;
        for code in 0..=14u8 {
            let value = VehicleClass::try_from(code).unwrap();
            assert_eq!(value.to_value(), i16::from(code));
            assert_eq!(
                VehicleClass::try_from_value(&i16::from(code)).unwrap(),
                value
            );
        }
        assert!(VehicleClass::try_from_value(&15).is_err());
    }
}

/// Serde adapter for upstream categorical codes, distinct from public named JSON.
pub(crate) mod numeric {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<T: Copy + Into<u8>, S: Serializer>(
        value: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8((*value).into())
    }

    pub fn deserialize<'de, T: TryFrom<u8>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<T, D::Error>
    where
        T::Error: std::fmt::Display,
    {
        T::try_from(u8::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

pub(crate) mod optional_numeric {
    use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error};

    #[allow(
        clippy::ref_option,
        reason = "Serde with adapters require a reference to the field"
    )]
    pub fn serialize<T: Copy + Into<u8>, S: Serializer>(
        value: &Option<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Into::into).serialize(serializer)
    }

    pub fn deserialize<'de, T: TryFrom<u8>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<T>, D::Error>
    where
        T::Error: std::fmt::Display,
    {
        Option::<u8>::deserialize(deserializer)?
            .map(T::try_from)
            .transpose()
            .map_err(D::Error::custom)
    }
}
