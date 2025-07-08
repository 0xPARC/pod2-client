#![deny(clippy::all)]

#[macro_use]
extern crate napi_derive;

use pod2::frontend::MainPod as Pod2MainPod;
use serde_json::Value as JsonValue;

#[napi]
#[allow(unused)]
pub struct MainPod {
  inner: Pod2MainPod,
}

#[napi]
impl MainPod {
  #[napi(factory)]
  pub fn deserialize(serialized_pod: String) -> Self {
    let main_pod: Pod2MainPod = serde_json::from_str(serialized_pod.as_str()).unwrap();
    MainPod { inner: main_pod }
  }

  #[napi]
  pub fn verify(&self) -> bool {
    self.inner.pod.verify().is_ok()
  }

  #[napi]
  pub fn public_statements(&self) -> JsonValue {
    serde_json::to_value(self.inner.pod.pub_statements()).unwrap()
  }
}
