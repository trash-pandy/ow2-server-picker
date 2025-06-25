use std::collections::HashMap;
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::net::IpAddr;
use std::sync::LazyLock;

use ipnetwork::IpNetwork;
use iter_tools::Itertools;

///// sourced from https://github.com/stowmyy/dropship/blob/main/dropship/dropship/src/core/Settings.h#L64
/// find: static const std::string (.*?) \{ (".*?") \};
/// repl: const $1: &str = $2;
//////////////
// put here //
//////////////
const GPC_ASIA_SOUTHEAST1: &str = "34.1.128.0/20,34.1.192.0/20,34.2.16.0/20,34.2.128.0/17,34.21.128.0/17,34.87.0.0/17,34.87.128.0/18,34.104.58.0/23,34.104.106.0/23,34.124.42.0/23,34.124.128.0/17,34.126.64.0/18,34.126.128.0/18,34.128.44.0/23,34.128.60.0/23,34.142.128.0/17,34.143.128.0/17,34.152.104.0/23,34.153.40.0/23,34.153.232.0/23,34.157.82.0/23,34.157.88.0/23,34.157.210.0/23,34.177.72.0/23,35.185.176.0/20,35.186.144.0/20,35.187.224.0/19,35.197.128.0/19,35.198.192.0/18,35.213.128.0/18,35.220.24.0/23,35.234.192.0/20,35.240.128.0/17,35.242.24.0/23,35.247.128.0/18,2600:1900:4080::/44";
const GPC_EUROPE_NORTH1: &str = "34.88.0.0/16,34.104.96.0/21,34.124.32.0/21,35.203.232.0/21,35.217.0.0/18,35.220.26.0/24,35.228.0.0/16,35.242.26.0/24,2600:1900:4150::/44";
const GPC_SOUTHAMERICA_EAST1: &str = "34.39.128.0/17,34.95.128.0/17,34.104.80.0/21,34.124.16.0/21,34.151.0.0/18,34.151.192.0/18,35.198.0.0/18,35.199.64.0/18,35.215.192.0/18,35.220.40.0/24,35.235.0.0/20,35.242.40.0/24,35.247.192.0/18,2600:1900:40f0::/44";
const GPC_ASIA_NORTHEAST1: &str = "34.84.0.0/16,34.85.0.0/17,34.104.62.0/23,34.104.128.0/17,34.127.190.0/23,34.146.0.0/16,34.157.64.0/20,34.157.164.0/22,34.157.192.0/20,35.187.192.0/19,35.189.128.0/19,35.190.224.0/20,35.194.96.0/19,35.200.0.0/17,35.213.0.0/17,35.220.56.0/22,35.221.64.0/18,35.230.240.0/20,35.242.56.0/22,35.243.64.0/18,104.198.80.0/20,104.198.112.0/20,2600:1900:4050::/44";
const GPC_ME_CENTRAL2: &str = "34.1.48.0/20,34.152.84.0/23,34.152.102.0/24,34.166.0.0/16,34.177.48.0/23,34.177.70.0/24,2600:1900:5400::/44";

const BLIZZARD_DACOM_KR: &str = "110.45.208.0/24,117.52.6.0/24,117.52.26.0/23,117.52.28.0/23,117.52.33.0/24,117.52.34.0/23,117.52.36.0/23,121.254.137.0/24,121.254.206.0/23,121.254.218.0/24,182.162.31.0/24";
//////////////
// put here //
//////////////

///// sourced from https://github.com/stowmyy/dropship/blob/main/dropship/dropship/src/core/Settings.h#L87
/// find: \{ (".*?"), \{ \.block = (".*?") \} \}
/// repl: ($1, $2)
static PREFIXES: LazyLock<HashMap<&str, &str>> = LazyLock::new(|| {
    [
        //////////////
        // put here //
        //////////////
        // { "test", { .block = "" }}, // PEERINGDB

        /* ord1
            - 24.105.40.0/21 seems to be the main one, worked fine for a while
            - started connecting to 64.224.0.0/21
                . 64.224.1.243
        */
        ("blizzard/ord1", "64.224.0.0/21,24.105.40.0/21"),
        /* las1
            - previous version also had some 24. servers. probably was lax1 (rip)
        */
        ("blizzard/las1", "64.224.24.0/23"),
        /* gen1
            -
        */
        ("google/europe-north1", GPC_EUROPE_NORTH1),
        /* gsg1
            -
        */
        ("google/asia-southeast1", GPC_ASIA_SOUTHEAST1),
        /* gbr1
            -
        */
        ("google/southamerica-east1", GPC_SOUTHAMERICA_EAST1),
        /* gtk1
            -
        */
        ("google/asia-northeast1", GPC_ASIA_NORTHEAST1),
        /* gmec2
            -
        */
        ("google/me-central2", GPC_ME_CENTRAL2),
        /* icn1
            - the two cidrs are the only ones i've ever connected to, they seem to work fine
            - kr is unique so for saftey i'm blocking all dacom cidrs from blizzard's asn
        */
        // ("blizzard/icn1", "121.254.206.0/23,117.52.26.0/23"),
        ("blizzard/icn1", BLIZZARD_DACOM_KR),
        /* syd2
            -
        */
        ("blizzard/syd2", "158.115.196.0/23"),
        /* tpe1
            - TROUBLESHOOTING: 5.42.164.0/22 is also another tpe server, never connected to it
        */
        ("blizzard/tpe1", "5.42.160.0/22,5.42.164.0/22"),
        /* ams1
            -
        */
        ("blizzard/ams1", "64.224.26.0/23"),
        //////////////
        // put here //
        //////////////
    ]
    .iter()
    .copied()
    .collect()
});

///// sourced from https://github.com/stowmyy/dropship/blob/main/dropship/dropship/src/core/Settings.h#L150-L204
/// find: \{ (.*?), \{[\n\s]*\.description = (.*?),[\n\s]*\.ip_ping = (.*)?,[\n\s]*.*\{ (.*?) \},[\n\s]*\} \},
/// repl: ($4, ($1, $2, $3)),
static META: LazyLock<HashMap<&str, (&str, &str, &str)>> = LazyLock::new(|| {
    [
        //////////////
        // put here //
        //////////////
        ("blizzard/ord1", ("USA - Central", "ORD1", "8.34.210.23")),
        ("blizzard/las1", ("USA - West", "LAS1", "34.16.128.42")),
        ("google/europe-north1", ("Finland", "GEN1", "34.88.0.1")),
        (
            "google/asia-southeast1",
            ("Singapore", "GSG1", "34.1.128.4"),
        ),
        (
            "google/southamerica-east1",
            ("Brazil", "GBR1", "34.39.128.0"),
        ),
        ("google/asia-northeast1", ("Tokyo", "GTK1", "34.84.0.0")),
        (
            "google/me-central2",
            ("Saudi Arabia", "GMEC2", "34.166.0.84"),
        ),
        ("blizzard/icn1", ("South Korea", "ICN1", "34.64.64.15")),
        ("blizzard/syd2", ("Australia", "SYD2", "34.40.128.34")),
        ("blizzard/tpe1", ("Taiwan", "TPE1", "34.80.0.0")),
        ("blizzard/ams1", ("Netherlands", "AMS1", "137.221.78.60")),
        //////////////
        // put here //
        //////////////
    ]
    .iter()
    .copied()
    .collect()
});

pub fn load() -> Vec<Region> {
    let mut blocks = Vec::with_capacity(PREFIXES.len());
    for &key in PREFIXES.keys() {
        let &prefix = PREFIXES.get(key).unwrap();
        let &(region, code, addr) = META.get(key).unwrap();

        blocks.push(Region {
            key: key.to_string(),
            name: region.to_string(),
            code: code.to_string(),
            ping: addr.parse().unwrap(),
            prefixes: prefix.split(",").map(|v| v.parse().unwrap()).collect_vec(),
        });
    }
    blocks
}

#[derive(Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct Region {
    pub key: String,
    pub name: String,
    pub code: String,
    pub ping: IpAddr,
    pub prefixes: Vec<IpNetwork>,
}

impl Hash for Region {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.key.hash(state);
    }
}

impl Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.key)
    }
}
