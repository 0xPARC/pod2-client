use std::collections::HashMap;

use num::traits::Euclid;
use pod2::{
    frontend::{MainPod, SignedDict},
    middleware::{Key, RawValue, TypedValue, Value},
};
use pod2_db::store::{PodData, PodInfo};
use serde::Serialize;

use crate::frog::{rarity_for, JUNK_RARITY};

#[derive(Serialize)]
#[serde(untagged)]
pub enum SerializablePod {
    Signed(SignedDict),
    Main(MainPod),
}

#[derive(Clone, Copy)]
pub enum DropTable {
    Search,
    Mine,
}

/// The data that is extracted out of the pod when it is first parsed, before
/// it is paired with a description pod.
pub struct FrogPodInfo {
    pub id: RawValue,
    pub leveled_up: bool,
    pub drop_table: DropTable,
    pub pod: SerializablePod,
}

pub trait IntoFrogPod: Clone {
    fn info(self) -> Option<FrogPodInfo>;
    fn pod_data(self) -> PodData;
}

impl IntoFrogPod for SignedDict {
    fn info(self) -> Option<FrogPodInfo> {
        let drop_table = match self.get("biome")?.typed() {
            TypedValue::Int(0) => DropTable::Search,
            TypedValue::Int(1) => DropTable::Mine,
            _ => return None,
        };
        let id = RawValue::from(self.dict.commitment());
        Some(FrogPodInfo {
            id,
            leveled_up: false,
            drop_table,
            pod: SerializablePod::Signed(self),
        })
    }
    fn pod_data(self) -> PodData {
        PodData::Signed(Box::new(self.into()))
    }
}

impl IntoFrogPod for MainPod {
    fn info(self) -> Option<FrogPodInfo> {
        /*
        let statements = self.pod.pub_statements();
        let (base_id, level) = statements
            .into_iter()
            .filter_map(|st| match st {
                Statement::Custom(cpr, args) if cpr.index == 0 => {
                    let base_id = args.first()?.raw();
                    let level: i64 = args.get(1)?.typed().try_into().ok()?;
                    Some((base_id, level))
                }
                _ => None,
            })
            .next()?;
        let pod_id = RawValue::from(self.id().0);
        Some(FrogPodInfo {
            pod_id,
            base_id,
            biome: 1,
            level,
            pod: SerializablePod::Main(self),
        })
        */
        //todo!()
        None
    }
    fn pod_data(self) -> PodData {
        PodData::Main(Box::new(self.into()))
    }
}

pub fn get_frog_pod_info(pod: PodInfo) -> Option<FrogPodInfo> {
    match pod.data {
        PodData::Signed(s) => {
            let inner = SignedDict::try_from(s.as_ref().clone()).ok()?;
            inner.info()
        }
        PodData::Main(s) => {
            let inner = MainPod::try_from(s.as_ref().clone()).ok()?;
            inner.info()
        }
    }
}

pub trait AsTyped {
    fn as_str(&self) -> Option<&str>;
    fn as_string(&self) -> Option<String>;
    fn as_int(&self) -> Option<i64>;
    fn as_bool(&self) -> Option<bool>;
    fn as_dictionary(&self) -> Option<HashMap<Key, Value>>;
}

impl AsTyped for Value {
    fn as_str(&self) -> Option<&str> {
        match self.typed() {
            TypedValue::String(s) => Some(s),
            _ => None,
        }
    }

    fn as_int(&self) -> Option<i64> {
        match self.typed() {
            TypedValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self.typed() {
            TypedValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_string(&self) -> Option<String> {
        match self.typed() {
            TypedValue::String(s) => Some(s.clone()),
            _ => None,
        }
    }

    fn as_dictionary(&self) -> Option<HashMap<Key, Value>> {
        match self.typed() {
            TypedValue::Dictionary(d) => Some(d.kvs().clone()),
            _ => None,
        }
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct FrogedexData {
    pub frog_id: i64,
    pub rarity: i64,
    #[serde(skip)]
    pub seed_range: HashMap<Key, Value>,
    pub name: String,
    pub image_url: String,
    pub description: String,
    pub seen: bool,
}

impl FrogedexData {
    pub fn from_pod(desc: &SignedDict) -> Option<Self> {
        let frog_id = desc.get("frog_id")?.as_int()?;
        Some(Self {
            frog_id,
            rarity: rarity_for(frog_id),
            seed_range: desc.get("seed_range")?.as_dictionary()?,
            name: desc.get("name")?.as_string()?,
            image_url: desc.get("image_url")?.as_string()?,
            description: desc.get("description")?.as_string()?,
            seen: true,
        })
    }
}

fn description_matches(frog: &FrogPodInfo, desc: &FrogedexData) -> bool {
    let table_str = Key::from((frog.drop_table as usize).to_string());
    if let Some(value) = desc.seed_range.get(&table_str) {
        let raw = value.raw().0;
        let offset = if frog.leveled_up { 2 } else { 0 };
        let seed = frog.id.0[0].0;
        //let b = (raw[offset].0..=raw[offset + 1].0).contains(&seed);
        //println!("{offset} {raw:?} {seed} {b}");
        (raw[offset].0..=raw[offset + 1].0).contains(&seed)
    } else {
        false
    }
}

pub fn description_for<'a>(
    frog: &'_ FrogPodInfo,
    descs: &'a [FrogedexData],
) -> Option<&'a FrogedexData> {
    descs
        .iter()
        .filter(|desc| description_matches(frog, desc))
        .next()
}

/// Frog data that is derived from the description pod.
#[derive(Serialize, Debug)]
pub struct FrogDerived {
    #[serde(flatten)]
    desc: FrogedexData,
    #[serde(flatten)]
    stats: FrogStats,
}

#[derive(Serialize, Debug)]
pub struct FrogStats {
    jump: u64,
    speed: u64,
    intelligence: u64,
    beauty: u64,
    temperament: u64,
}

const DEFAULT_TEMPERAMENTS: [u64; 7] = [2, 3, 4, 7, 10, 16, 18];

pub fn compute_frog_stats(id: RawValue, desc: &FrogedexData) -> FrogStats {
    let val = id.0[1].0;
    match desc.rarity {
        JUNK_RARITY => FrogStats {
            jump: 0,
            speed: 0,
            intelligence: 0,
            beauty: val.rem_euclid(8) + 8,
            temperament: 1,
        },
        _ => {
            let bonus = 0;
            let (val, temperament_index) = val.div_rem_euclid(&7);
            let temperament = DEFAULT_TEMPERAMENTS[temperament_index as usize];
            let (val, beauty) = val.div_rem_euclid(&8);
            let (val, intelligence) = val.div_rem_euclid(&8);
            let (val, speed) = val.div_rem_euclid(&8);
            let jump = val.rem_euclid(8);
            FrogStats {
                jump: jump + bonus,
                speed: speed + bonus,
                intelligence: intelligence + bonus,
                beauty: beauty + bonus,
                temperament,
            }
        }
    }
}

impl FrogDerived {
    pub fn from_info(pod: &FrogPodInfo, desc: &FrogedexData) -> Self {
        let stats = compute_frog_stats(pod.id, desc);
        Self {
            desc: desc.clone(),
            stats,
        }
    }
}

/// Frog data in a form that the UI can use.
#[derive(Serialize, Debug)]
pub struct Frog {
    pub id: String,
    pub derived: Option<FrogDerived>,
    pub offer_level_up: bool,
}
