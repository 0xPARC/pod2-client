use pod2::frontend::{SerializedMainPod, SerializedSignedPod};
use pod2_db::store::{PodInfo, SpaceInfo};
use schemars::{JsonSchema, schema_for};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct JsonTypes {
    main_pod: SerializedMainPod,
    signed_pod: SerializedSignedPod,
    pod_info: PodInfo,
    space_info: SpaceInfo,
}

fn main() {
    let combined_schema = schema_for!(JsonTypes);
    match serde_json::to_string_pretty(&combined_schema) {
        Ok(json) => println!("{json}"),
        Err(e) => eprintln!("Error serializing combined schema: {e}"),
    }
}
