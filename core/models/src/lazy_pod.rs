use std::sync::OnceLock;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::marker::PhantomData;

/// A generic lazy deserialization wrapper that stores the serialized value and deserializes only when accessed
#[derive(Debug, Clone)]
pub struct LazyDeser<T> {
    /// The pre-serialized JSON value - this is what gets output during serialization
    serialized_value: serde_json::Value,
    /// Lazily deserialized typed value
    value: OnceLock<T>,
    _phantom: PhantomData<T>,
}

// Serialize implementation - just output the stored serialized value directly
impl<T> Serialize for LazyDeser<T>
where
    T: DeserializeOwned + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Just serialize the pre-stored value directly - no parsing or re-serialization
        self.serialized_value.serialize(serializer)
    }
}

// Deserialize implementation - handle both direct JSON values and JSON strings
impl<'de, T> Deserialize<'de> for LazyDeser<T>
where
    T: DeserializeOwned + Serialize,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;
        
        // First try to deserialize as a JSON value
        let value = serde_json::Value::deserialize(deserializer)?;
        
        // If it's a string, try to parse it as JSON (for database storage)
        let serialized_value = if let serde_json::Value::String(json_str) = value {
            // Parse the JSON string to get the actual JSON object
            serde_json::from_str(&json_str).map_err(D::Error::custom)?
        } else {
            // It's already a JSON value, use it directly
            value
        };
        
        Ok(Self::new(serialized_value))
    }
}

impl<T> LazyDeser<T>
where
    T: DeserializeOwned + Serialize,
{
    pub fn new(serialized_value: serde_json::Value) -> Self {
        Self {
            serialized_value,
            value: OnceLock::new(),
            _phantom: PhantomData,
        }
    }

    pub fn from_json_string(json: String) -> Result<Self, serde_json::Error> {
        let serialized_value = serde_json::from_str(&json)?;
        Ok(Self::new(serialized_value))
    }

    pub fn from_value(value: T) -> Result<Self, serde_json::Error> {
        let serialized_value = serde_json::to_value(&value)?;
        let lazy_deser = Self::new(serialized_value);
        // Pre-populate the cache
        let _ = lazy_deser.value.set(value);
        Ok(lazy_deser)
    }

    pub fn get(&self) -> Result<&T, serde_json::Error> {
        Ok(self.value.get_or_init(|| {
            serde_json::from_value(self.serialized_value.clone()).unwrap_or_else(|e| {
                // If deserialization fails, we need to handle it gracefully
                // For now, panic - in production you might want different error handling
                panic!("Failed to deserialize JSON in LazyDeser. JSON content: '{}', Error: {}", self.serialized_value, e)
            })
        }))
    }

    pub fn try_get(&self) -> Result<&T, serde_json::Error> {
        match self.value.get() {
            Some(value) => Ok(value),
            None => {
                let value = serde_json::from_value(self.serialized_value.clone())?;
                // Try to set the value, but don't worry if it fails (race condition)
                let _ = self.value.set(value);
                // Get the value again (either our value or one from a concurrent thread)
                Ok(self.value.get().unwrap())
            }
        }
    }

    pub fn json(&self) -> String {
        serde_json::to_string(&self.serialized_value).unwrap_or_else(|_| "{}".to_string())
    }

    pub fn serialized_value(&self) -> &serde_json::Value {
        &self.serialized_value
    }
}