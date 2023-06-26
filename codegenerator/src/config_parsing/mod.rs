use std::error::Error;
use std::path::PathBuf;

use ethers::abi::{Event as EthAbiEvent, HumanReadableParser};
use serde::{Deserialize, Serialize};

use crate::hbs_templating::codegen_templates::SyncConfigTemplate;
use crate::project_paths::handler_paths::ContractUniqueId;
use crate::{
    capitalization::{Capitalize, CapitalizedOptions},
    project_paths::ParsedPaths,
};

mod defaults;
pub mod entity_parsing;
pub mod event_parsing;

type NetworkId = i32;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct RequiredEntity {
    name: String,
    labels: Vec<String>,
}

#[derive(Debug, PartialEq, Deserialize, Clone, Serialize)]
#[serde(try_from = "String")]
enum EventNameOrSig {
    Name(String),
    Event(EthAbiEvent),
}

impl TryFrom<String> for EventNameOrSig {
    type Error = String;

    fn try_from(event_string: String) -> Result<Self, Self::Error> {
        let parse_event_sig = |sig: &str| -> Result<EthAbiEvent, Self::Error> {
            match HumanReadableParser::parse_event(sig) {
                Ok(event) => Ok(event),
                Err(err) => Err(format!(
                    "Unable to parse event signature {} due to the following error: {}",
                    sig, err
                )),
            }
        };

        let trimmed = event_string.trim();

        let name_or_sig = if trimmed.starts_with("event ") {
            let parsed_event = parse_event_sig(trimmed)?;
            EventNameOrSig::Event(parsed_event)
        } else if trimmed.contains("(") {
            let signature = format!("event {}", trimmed);
            let parsed_event = parse_event_sig(&signature)?;
            EventNameOrSig::Event(parsed_event)
        } else {
            EventNameOrSig::Name(trimmed.to_string())
        };

        Ok(name_or_sig)
    }
}

impl EventNameOrSig {
    pub fn get_name(&self) -> String {
        match self {
            EventNameOrSig::Name(name) => name.to_owned(),
            EventNameOrSig::Event(event) => event.name.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ConfigEvent {
    event: EventNameOrSig,
    required_entities: Option<Vec<RequiredEntity>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Network {
    pub id: NetworkId,
    rpc_config: RpcConfig,
    start_block: i32,
    pub contracts: Vec<ConfigContract>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct RpcConfig {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_block_interval: Option<u32>,
    // After an RPC error, how much to scale back the number of blocks requested at once
    #[serde(skip_serializing_if = "Option::is_none")]
    backoff_multiplicative: Option<f32>,
    // Without RPC errors or timeouts, how much to increase the number of blocks requested by for the next batch
    #[serde(skip_serializing_if = "Option::is_none")]
    acceleration_additive: Option<u32>,
    // Do not further increase the block interval past this limit
    #[serde(skip_serializing_if = "Option::is_none")]
    interval_ceiling: Option<u32>,
    // After an error, how long to wait before retrying
    #[serde(skip_serializing_if = "Option::is_none")]
    backoff_millis: Option<u32>,
    // How long to wait before cancelling an RPC request
    #[serde(skip_serializing_if = "Option::is_none")]
    query_timeout_millis: Option<u32>,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct ConfigContract {
    pub name: String,
    // Eg for implementing a custom deserializer
    //  #[serde(deserialize_with = "abi_path_to_abi")]
    pub abi_file_path: Option<String>,
    pub handler: String,
    address: NormalizedList<String>,
    events: Vec<ConfigEvent>,
}

// We require this intermediate struct in order to allow the config to skip specifying "address".
#[derive(Deserialize)]
struct IntermediateConfigContract {
    pub name: String,
    pub abi_file_path: Option<String>,
    pub handler: String,
    // This is the difference - adding Option<> around it.
    address: Option<NormalizedList<String>>,
    events: Vec<ConfigEvent>,
}

impl From<IntermediateConfigContract> for ConfigContract {
    fn from(icc: IntermediateConfigContract) -> Self {
        ConfigContract {
            name: icc.name,
            abi_file_path: icc.abi_file_path,
            handler: icc.handler,
            address: icc.address.unwrap_or(NormalizedList { inner: vec![] }),
            events: icc.events,
        }
    }
}

impl<'de> Deserialize<'de> for ConfigContract {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        IntermediateConfigContract::deserialize(deserializer).map(ConfigContract::from)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(untagged)]
enum SingleOrList<T: Clone> {
    Single(T),
    List(Vec<T>),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct OptSingleOrList<T: Clone>(Option<SingleOrList<T>>);

impl<T: Clone> OptSingleOrList<T> {
    fn to_normalized_list(&self) -> NormalizedList<T> {
        let list: Vec<T> = match &self.0 {
            Some(single_or_list) => match single_or_list {
                SingleOrList::Single(val) => vec![val.clone()],
                SingleOrList::List(list) => list.to_vec(),
            },
            None => Vec::new(),
        };

        NormalizedList { inner: list }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(try_from = "OptSingleOrList<T>")]
struct NormalizedList<T: Clone> {
    inner: Vec<T>,
}

impl<T: Clone> NormalizedList<T> {
    #[cfg(test)]
    fn from(list: Vec<T>) -> Self {
        NormalizedList { inner: list }
    }

    #[cfg(test)]
    fn from_single(val: T) -> Self {
        Self::from(vec![val])
    }
}

impl<T: Clone> TryFrom<OptSingleOrList<T>> for NormalizedList<T> {
    type Error = String;

    fn try_from(single_or_list: OptSingleOrList<T>) -> Result<Self, Self::Error> {
        Ok(single_or_list.to_normalized_list())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncConfigUnstable {
    initial_block_interval: Option<u32>,
    // After an RPC error, how much to scale back the number of blocks requested at once
    backoff_multiplicative: Option<f32>,
    // Without RPC errors or timeouts, how much to increase the number of blocks requested by for the next batch
    acceleration_additive: Option<u32>,
    // Do not further increase the block interval past this limit
    interval_ceiling: Option<u32>,
    // After an error, how long to wait before retrying
    backoff_millis: Option<u32>,
    // How long to wait before cancelling an RPC request
    query_timeout_millis: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(non_snake_case)] //Allow unstable__sync_config to be non_snake_case§
pub struct Config {
    name: String,
    version: String,
    description: String,
    repository: String,
    pub schema: Option<String>,
    pub networks: Vec<Network>,
    // Make it very clear that this config is not stabilized yet
    pub unstable__sync_config: Option<SyncConfigUnstable>,
}

// fn abi_path_to_abi<'de, D>(deserializer: D) -> Result<u64, D::Error>
// where
//     D: Deserializer<'de>,
// {
//     let abi_file_path: &str = Deserialize::deserialize(deserializer)?;
//     // ... convert to abi here
// }

type StringifiedAbi = String;
type EthAddress = String;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
struct ContractTemplate {
    name: CapitalizedOptions,
    abi: StringifiedAbi,
    addresses: Vec<EthAddress>,
    events: Vec<CapitalizedOptions>,
}

#[derive(Debug, Serialize, PartialEq, Clone)]
pub struct ChainConfigTemplate {
    network_config: Network,
    contracts: Vec<ContractTemplate>,
}

pub fn deserialize_config_from_yaml(config_path: &PathBuf) -> Result<Config, Box<dyn Error>> {
    let config = std::fs::read_to_string(&config_path).map_err(|err| {
        format!(
            "Failed to resolve config path {} with Error {}",
            &config_path.to_str().unwrap_or("unknown path"),
            err.to_string()
        )
    })?;

    let deserialized_yaml: Config = serde_yaml::from_str(&config).map_err(|err| {
        format!(
            "Failed to deserialize config with Error {}",
            err.to_string()
        )
    })?;
    Ok(deserialized_yaml)
}

pub fn convert_config_to_chain_configs(
    parsed_paths: &ParsedPaths,
) -> Result<Vec<ChainConfigTemplate>, Box<dyn Error>> {
    let config = deserialize_config_from_yaml(&parsed_paths.project_paths.config)?;

    let mut chain_configs = Vec::new();
    for network in config.networks.iter() {
        let mut contract_templates = Vec::new();

        for contract in network.contracts.iter() {
            let contract_unique_id = ContractUniqueId {
                network_id: network.id,
                name: contract.name.clone(),
            };

            let parsed_abi_from_file = parsed_paths.get_contract_abi(&contract_unique_id)?;

            let mut reduced_abi = ethers::abi::Contract::default();

            for config_event in contract.events.iter() {
                let abi_event = match &config_event.event {
                    EventNameOrSig::Name(config_event_name) => match &parsed_abi_from_file {
                        Some(contract_abi) => {
                            let format_err = |err| -> String {
                                format!("event \"{}\" cannot be parsed the provided abi for contract {} due to error: {:?}", config_event_name, contract.name, err)
                            };
                            contract_abi.event(&config_event_name).map_err(format_err)?
                        }
                        None => {
                            let message = format!("Please add abi_file_path for contract {} to your config to parse event {} or define the signature in the config", contract.name, config_event_name);
                            Err(message)?
                        }
                    },
                    EventNameOrSig::Event(abi_event) => abi_event,
                };

                reduced_abi
                    .events
                    .entry(abi_event.name.clone())
                    .or_default()
                    .push(abi_event.clone());
            }

            let stringified_abi = serde_json::to_string(&reduced_abi)?;
            let contract_template = ContractTemplate {
                name: contract.name.to_capitalized_options(),
                abi: stringified_abi,
                addresses: contract.address.inner.clone(),
                events: contract
                    .events
                    .iter()
                    .map(|config_event| config_event.event.get_name().to_capitalized_options())
                    .collect(),
            };
            contract_templates.push(contract_template);
        }
        let chain_config = ChainConfigTemplate {
            network_config: network.clone(),
            contracts: contract_templates,
        };
        chain_configs.push(chain_config);
    }
    Ok(chain_configs)
}

pub fn convert_config_to_sync_config(
    parsed_paths: &ParsedPaths,
) -> Result<SyncConfigTemplate, Box<dyn Error>> {
    let config = deserialize_config_from_yaml(&parsed_paths.project_paths.config)?;
    let c = config.unstable__sync_config.as_ref();

    let d = defaults::SYNC_CONFIG;

    let sync_config = SyncConfigTemplate {
        initial_block_interval: c
            .and_then(|c| c.initial_block_interval)
            .unwrap_or(d.initial_block_interval),
        backoff_multiplicative: c
            .and_then(|c| c.backoff_multiplicative)
            .unwrap_or(d.backoff_multiplicative),
        acceleration_additive: c
            .and_then(|c| c.acceleration_additive)
            .unwrap_or(d.acceleration_additive),
        interval_ceiling: c
            .and_then(|c| c.interval_ceiling)
            .unwrap_or(d.interval_ceiling),
        backoff_millis: c.and_then(|c| c.backoff_millis).unwrap_or(d.backoff_millis),
        query_timeout_millis: c
            .and_then(|c| c.query_timeout_millis)
            .unwrap_or(d.query_timeout_millis),
    };

    Ok(sync_config)
}

pub fn get_project_name_from_config(parsed_paths: &ParsedPaths) -> Result<String, Box<dyn Error>> {
    let config = deserialize_config_from_yaml(&parsed_paths.project_paths.config)?;
    Ok(config.name)
}
#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use ethers::abi::{Event, EventParam, ParamType};

    use crate::capitalization::Capitalize;
    use crate::config_parsing::{EventNameOrSig, NormalizedList};
    use crate::{cli_args::ProjectPathsArgs, project_paths::ParsedPaths};

    use super::ChainConfigTemplate;

    #[test]
    fn deserialize_address() {
        let no_address = r#"null"#;
        let deserialized: NormalizedList<String> = serde_json::from_str(no_address).unwrap();
        assert_eq!(deserialized, NormalizedList::from(vec![]));

        let single_address = r#""0x123""#;
        let deserialized: NormalizedList<String> = serde_json::from_str(single_address).unwrap();
        assert_eq!(
            deserialized,
            NormalizedList::from(vec!["0x123".to_string()])
        );

        let multi_address = r#"["0x123", "0x456"]"#;
        let deserialized: NormalizedList<String> = serde_json::from_str(multi_address).unwrap();
        assert_eq!(
            deserialized,
            NormalizedList::from(vec!["0x123".to_string(), "0x456".to_string()])
        );
    }

    #[test]
    fn convert_to_chain_configs_case_1() {
        let address1 = String::from("0x2E645469f354BB4F5c8a05B3b30A929361cf77eC");
        let abi_file_path = PathBuf::from("test/abis/Contract1.json");

        let event1 = super::ConfigEvent {
            event: EventNameOrSig::Name(String::from("NewGravatar")),
            required_entities: None,
        };

        let event2 = super::ConfigEvent {
            event: EventNameOrSig::Name(String::from("UpdatedGravatar")),
            required_entities: None,
        };

        let contract1 = super::ConfigContract {
            handler: "./src/EventHandler.js".to_string(),
            address: NormalizedList::from_single(address1.clone()),
            name: String::from("Contract1"),
            //needed to have relative path in order to match config1.yaml
            abi_file_path: Some(String::from("../abis/Contract1.json")),
            events: vec![event1.clone(), event2.clone()],
        };

        let contracts = vec![contract1.clone()];
        
        let rpc_config1 = super::RpcConfig {
            url: String::from("https://eth.com"),
            initial_block_interval: Some(10000),
            interval_ceiling: Some(10000),
            backoff_multiplicative: None,
            acceleration_additive: None,
            backoff_millis: None,
            query_timeout_millis: None,
        };

        let network1 = super::Network {
            id: 1,
            rpc_config: rpc_config1,
            start_block: 0,
            contracts,
        };

        let project_root = String::from("test");
        let config = String::from("configs/config1.yaml");
        let generated = String::from("generated/");
        let parsed_paths = ParsedPaths::new(ProjectPathsArgs {
            project_root,
            config,
            generated,
        })
        .unwrap();
        let chain_configs = super::convert_config_to_chain_configs(&parsed_paths).unwrap();
        let abi_unparsed_string =
            fs::read_to_string(abi_file_path).expect("expected json file to be at this path");
        let abi_parsed: ethers::abi::Contract = serde_json::from_str(&abi_unparsed_string).unwrap();
        let abi_parsed_string = serde_json::to_string(&abi_parsed).unwrap();
        let contract1 = super::ContractTemplate {
            name: String::from("Contract1").to_capitalized_options(),
            abi: abi_parsed_string,
            addresses: vec![address1.clone()],
            events: vec![
                event1.event.get_name().to_capitalized_options(),
                event2.event.get_name().to_capitalized_options(),
            ],
        };

        let chain_config_1 = ChainConfigTemplate {
            network_config: network1,
            contracts: vec![contract1],
        };

        let expected_chain_configs = vec![chain_config_1];

        assert_eq!(
            expected_chain_configs[0].network_config,
            chain_configs[0].network_config
        );
        assert_eq!(expected_chain_configs, chain_configs,);
    }

    #[test]
    fn convert_to_chain_configs_case_2() {
        let address1 = String::from("0x2E645469f354BB4F5c8a05B3b30A929361cf77eC");
        let address2 = String::from("0x1E645469f354BB4F5c8a05B3b30A929361cf77eC");

        let abi_file_path = PathBuf::from("test/abis/Contract1.json");

        let event1 = super::ConfigEvent {
            event: EventNameOrSig::Name(String::from("NewGravatar")),
            required_entities: None,
        };

        let event2 = super::ConfigEvent {
            event: EventNameOrSig::Name(String::from("UpdatedGravatar")),
            required_entities: None,
        };

        let contract1 = super::ConfigContract {
            handler: "./src/EventHandler.js".to_string(),
            address: NormalizedList::from_single(address1.clone()),
            name: String::from("Contract1"),
            abi_file_path: Some(String::from("../abis/Contract1.json")),
            events: vec![event1.clone(), event2.clone()],
        };

        let contracts1 = vec![contract1.clone()];

        let rpc_config1 = super::RpcConfig {
            url: String::from("https://eth.com"),
            initial_block_interval: Some(10000),
            interval_ceiling: Some(10000),
            backoff_multiplicative: None,
            acceleration_additive: None,
            backoff_millis: None,
            query_timeout_millis: None,
        };

        let network1 = super::Network {
            id: 1,
            rpc_config: rpc_config1,
            start_block: 0,
            contracts: contracts1,
        };
        let contract2 = super::ConfigContract {
            handler: "./src/EventHandler.js".to_string(),
            address: NormalizedList::from_single(address2.clone()),
            name: String::from("Contract1"),
            abi_file_path: Some(String::from("../abis/Contract1.json")),
            events: vec![event1.clone(), event2.clone()],
        };

        let contracts2 = vec![contract2];

        let rpc_config2 = super::RpcConfig {
            url: String::from("https://eth.com"),
            initial_block_interval: Some(10000),
            interval_ceiling: Some(10000),
            backoff_multiplicative: None,
            acceleration_additive: None,
            backoff_millis: None,
            query_timeout_millis: None,
        };

        let network2 = super::Network {
            id: 2,
            rpc_config: rpc_config2,
            start_block: 0,
            contracts: contracts2,
        };

        let project_root = String::from("test");
        let config = String::from("configs/config2.yaml");
        let generated = String::from("generated/");
        let parsed_paths = ParsedPaths::new(ProjectPathsArgs {
            project_root,
            config,
            generated,
        })
        .unwrap();

        let chain_configs = super::convert_config_to_chain_configs(&parsed_paths).unwrap();

        let events = vec![
            event1.event.get_name().to_capitalized_options(),
            event2.event.get_name().to_capitalized_options(),
        ];

        let abi_unparsed_string =
            fs::read_to_string(abi_file_path).expect("expected json file to be at this path");
        let abi_parsed: ethers::abi::Contract = serde_json::from_str(&abi_unparsed_string).unwrap();
        let abi_parsed_string = serde_json::to_string(&abi_parsed).unwrap();
        let contract1 = super::ContractTemplate {
            name: String::from("Contract1").to_capitalized_options(),
            abi: abi_parsed_string.clone(),
            addresses: vec![address1.clone()],
            events: events.clone(),
        };
        let contract2 = super::ContractTemplate {
            name: String::from("Contract1").to_capitalized_options(),
            abi: abi_parsed_string.clone(),
            addresses: vec![address2.clone()],
            events,
        };

        let chain_config_1 = ChainConfigTemplate {
            network_config: network1,
            contracts: vec![contract1],
        };
        let chain_config_2 = ChainConfigTemplate {
            network_config: network2,
            contracts: vec![contract2],
        };

        let expected_chain_configs = vec![chain_config_1, chain_config_2];

        assert_eq!(chain_configs, expected_chain_configs);
    }

    #[test]
    fn deserializes_event_name() {
        let event_string = serde_json::to_string("MyEvent").unwrap();

        let name_or_sig = serde_json::from_str::<EventNameOrSig>(&event_string).unwrap();
        let expected = EventNameOrSig::Name("MyEvent".to_string());
        assert_eq!(name_or_sig, expected);
    }

    #[test]
    fn deserializes_event_sig_with_event_prefix() {
        let event_string = serde_json::to_string("event MyEvent(uint256 myArg)").unwrap();

        let name_or_sig = serde_json::from_str::<EventNameOrSig>(&event_string).unwrap();
        let expected_event = Event {
            name: "MyEvent".to_string(),
            anonymous: false,
            inputs: vec![EventParam {
                indexed: false,
                name: "myArg".to_string(),
                kind: ParamType::Uint(256),
            }],
        };
        let expected = EventNameOrSig::Event(expected_event);
        assert_eq!(name_or_sig, expected);
    }

    #[test]
    fn deserializes_event_sig_without_event_prefix() {
        let event_string = serde_json::to_string("MyEvent(uint256 myArg)").unwrap();

        let name_or_sig = serde_json::from_str::<EventNameOrSig>(&event_string).unwrap();
        let expected_event = Event {
            name: "MyEvent".to_string(),
            anonymous: false,
            inputs: vec![EventParam {
                indexed: false,
                name: "myArg".to_string(),
                kind: ParamType::Uint(256),
            }],
        };
        let expected = EventNameOrSig::Event(expected_event);
        assert_eq!(name_or_sig, expected);
    }

    #[test]
    #[should_panic]
    fn deserializes_event_sig_invalid_panics() {
        let event_string = serde_json::to_string("MyEvent(uint69 myArg)").unwrap();
        serde_json::from_str::<EventNameOrSig>(&event_string).unwrap();
    }
}
