use crate::components::login::AuthState;

use super::auth::{AuthAgent, AuthAgentRequest};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use sfi_core::core::{Inventory, Item};
use std::{
    collections::HashSet,
    ops::DerefMut,
    rc::Rc,
    sync::{Arc, RwLock},
};
use uuid::Uuid;
use yew::{
    format::Json,
    services::{storage::Area, StorageService},
    worker::*,
};

const EVENT_STORE_KEY: &'static str = "sfi.events.store";
const SIMPLE_STORE_KEY: &'static str = "sfi.simple_data.store";

#[derive(Debug)]
pub enum DataAgentRequest {
    MakeDebugInventory,

    GetInventories,
    GetInventory(Uuid),
    CreateInventory(String),
    UpdateInventory {
        target: Arc<RwLock<Inventory>>,
        name: String,
        owner: Uuid,
        admins: Vec<Uuid>,
        writables: Vec<Uuid>,
        readables: Vec<Uuid>,
    },
    DeleteInventory(Arc<RwLock<Inventory>>),

    UpdateItem {
        target: Arc<RwLock<Item>>,
        name: String,
        ean: Option<String>,
    },
    CreateItem(Uuid, String, Option<String>),
    DeleteAllData,
    GetItem(Uuid, Uuid),
    DeleteItem(Arc<RwLock<Item>>),
}

#[derive(Debug)]
pub enum DataAgentResponse {
    Inventories(Vec<Arc<RwLock<Inventory>>>),
    NewInventoryUuid(Uuid),
    Inventory(Arc<RwLock<Inventory>>),
    InvalidInventoryUuid,
    UpdatedInventory(Arc<RwLock<Inventory>>),
    DeletedInventory(Uuid),

    NewItemUuid(Uuid),
    Item(Arc<RwLock<Item>>),
    UpdatedItem,
    DeletedItem(Uuid),
}

pub enum Msg {
    NewAuthState(Rc<AuthState>),
}

pub struct DataAgent {
    link: AgentLink<DataAgent>,
    subscribers: HashSet<HandlerId>,
    local_storage: StorageService,
    auth_state: Rc<AuthState>,

    inventories: Vec<Arc<RwLock<Inventory>>>,
    auth_bridge: Box<dyn Bridge<AuthAgent>>,
}

impl Agent for DataAgent {
    type Reach = Context<Self>;
    type Message = Msg;
    type Input = DataAgentRequest;
    type Output = DataAgentResponse;

    fn create(link: AgentLink<Self>) -> Self {
        // Get a reference to localStorage
        let local_storage = StorageService::new(Area::Local).expect("Cannot use localStorage");

        // Load the event store from localStorage
        let store = {
            if let Json(Ok(store)) = local_storage.restore(SIMPLE_STORE_KEY) {
                // Load the event store from localStorage
                store
            } else {
                // If no such entry exists, create a new one
                vec![]
            }
        };

        // Initiate a bridge to the auth agent
        let mut auth_bridge = AuthAgent::bridge(link.callback(Msg::NewAuthState));

        // Request the current authentication status
        // auth_bridge.send(AuthAgentRequest::GetAuthStatus);

        Self {
            subscribers: HashSet::new(),
            inventories: store,
            local_storage,
            auth_state: Rc::new(AuthState::Initial),
            auth_bridge,
            link,
        }
    }

    fn update(&mut self, msg: Self::Message) {
        match msg {
            Msg::NewAuthState(auth_state) => self.auth_state = auth_state,
        };
    }

    fn handle_input(&mut self, msg: Self::Input, id: HandlerId) {
        match msg {
            DataAgentRequest::GetInventories => {
                // TODO remove these clones

                for sub in self.subscribers.iter() {
                    self.link.respond(
                        *sub,
                        DataAgentResponse::Inventories(self.inventories.clone()),
                    )
                }
            }
            DataAgentRequest::MakeDebugInventory => {
                let res = if let AuthState::LoggedIn(user_info) = self.auth_state.as_ref() {
                    let inv = Inventory::new("debug inv".to_string(), user_info.uuid);
                    let uuid = inv.uuid;
                    self.inventories.push(Arc::new(RwLock::new(inv)));
                    uuid
                } else {
                    let inv = Inventory::new("debug inv".to_string(), Uuid::new_v4());
                    let uuid = inv.uuid;
                    self.inventories.push(Arc::new(RwLock::new(inv)));
                    uuid
                };

                self.persist_data();

                self.link
                    .respond(id, DataAgentResponse::NewInventoryUuid(res));

                for sub in self.subscribers.iter() {
                    self.link.respond(
                        *sub,
                        DataAgentResponse::Inventories(self.inventories.to_vec().clone()),
                    )
                }
            }
            DataAgentRequest::CreateInventory(name) => {
                if let AuthState::LoggedIn(user_info) = self.auth_state.as_ref() {
                    let inv = Inventory::new(name, user_info.uuid);
                    let uuid = inv.uuid;
                    self.inventories.push(Arc::new(RwLock::new(inv)));

                    self.persist_data();

                    self.link
                        .respond(id, DataAgentResponse::NewInventoryUuid(uuid));

                    for sub in self.subscribers.iter() {
                        self.link.respond(
                            *sub,
                            DataAgentResponse::Inventories(self.inventories.to_vec().clone()),
                        )
                    }
                }
            }
            DataAgentRequest::DeleteAllData => {
                self.inventories = vec![];
                self.persist_data();

                let res = (&self.inventories).to_vec();

                for sub in self.subscribers.iter() {
                    self.link
                        .respond(*sub, DataAgentResponse::Inventories(res.clone()))
                }
            }
            DataAgentRequest::GetInventory(inv_uuid) => {
                let res = if let Some(inventory) = self.find_inv(inv_uuid) {
                    DataAgentResponse::Inventory(inventory.clone())
                } else {
                    DataAgentResponse::InvalidInventoryUuid
                };

                self.link.respond(id, res)
            }
            DataAgentRequest::CreateItem(inventory_uuid, name, ean) => {
                let res = {
                    let item = Item::new(inventory_uuid, name, ean);
                    let uuid = item.uuid;

                    self.find_inv(inventory_uuid)
                        .expect("No such inventory (cannot write)")
                        .write()
                        .expect("Cannot write inventory")
                        .items
                        .push(Arc::new(RwLock::new(item)));

                    self.persist_data();

                    DataAgentResponse::NewItemUuid(uuid)
                };

                self.link.respond(id, res)
            }
            DataAgentRequest::UpdateInventory {
                target,
                name,
                owner,
                admins,
                writables,
                readables,
            } => {
                let res = if let Ok(mut inventory) = target.write() {
                    inventory.name = name;
                    inventory.owner = owner;
                    inventory.admins = admins;
                    inventory.writables = writables;
                    inventory.readables = readables;

                    drop(inventory);

                    self.persist_data();

                    DataAgentResponse::UpdatedInventory(target.clone())
                } else {
                    DataAgentResponse::InvalidInventoryUuid
                };

                self.link.respond(id, res);
            }
            DataAgentRequest::GetItem(inventory_uuid, item_uuid) => {
                // let res = if let Some(item) = crate::find_item!(inventory_uuid, item_uuid) {
                //     DataAgentResponse::Item(item.clone())
                // } else {
                //     DataAgentResponse::InvalidInventoryUuid
                // };

                let res = if let Some(item) = {
                    self.inventories
                        .iter()
                        .find(|inv| {
                            inv.read().expect("Cannot read inventory uuid").uuid == inventory_uuid
                        })
                        .expect("No such item")
                        .read()
                        .expect("Cannot read inventory")
                        .items
                        .iter()
                        .find(|item| item.read().expect("Cannot read item").uuid == item_uuid)
                } {
                    DataAgentResponse::Item(item.clone())
                } else {
                    DataAgentResponse::InvalidInventoryUuid
                };

                self.link.respond(id, res)
            }
            DataAgentRequest::UpdateItem { target, name, ean } => {
                let res = if let Ok(mut item) = target.write() {
                    item.name = name;
                    item.ean = ean;

                    drop(item);

                    self.persist_data();

                    DataAgentResponse::UpdatedItem
                } else {
                    // TODO Maybe replace this with InvalidItemUuid or something; notice: the error could still be the inventory UUID
                    DataAgentResponse::InvalidInventoryUuid
                };

                self.link.respond(id, res);
            }
            DataAgentRequest::DeleteInventory(target) => {
                let target_uuid = target
                    .read()
                    .expect("Cannot read inventory to be deleted")
                    .uuid;

                let index = self
                    .inventories
                    .iter()
                    .position(|i| i.read().expect("Cannot read inventory").uuid == target_uuid)
                    .expect("No such inventory");

                self.inventories.remove(index);

                self.persist_data();

                let response = DataAgentResponse::DeletedInventory(target_uuid);
                self.link.respond(id, response);
            }
            DataAgentRequest::DeleteItem(target) => {
                let target = target.read().expect("Cannot read item to be deleted");

                let mut inventory = self
                    .inventories
                    .iter()
                    .find(|i| {
                        i.read().expect("Cannot read inventory").uuid == target.inventory_uuid
                    })
                    .expect("Cannot get inventory as mutable")
                    .write()
                    .expect("Cannot write to inventory");

                let item_index = inventory
                    .items
                    .iter()
                    .position(|i| i.read().expect("Cannot read inventory").uuid == target.uuid)
                    .expect("No such item");

                inventory.items.remove(item_index);

                drop(inventory);

                self.persist_data();

                let response = DataAgentResponse::DeletedItem(target.uuid);
                self.link.respond(id, response);
            }
        }
    }

    fn connected(&mut self, id: HandlerId) {
        // FIelD `1` oF STrucT `yeW::AGENT::hANnlERiD` Is PRivATE
        // PRiVATE fIELd rUsTC e0616
        // if id.1 {}
        if format!("{:?}", &id).contains("true") {
            self.subscribers.insert(id);
        }
    }

    fn disconnected(&mut self, id: HandlerId) {
        self.subscribers.remove(&id);
    }
}

impl DataAgent {
    fn persist_data(&mut self) -> () {
        self.local_storage
            .store(SIMPLE_STORE_KEY, Json(&self.inventories));
    }

    fn find_inv(&mut self, inv_uuid: Uuid) -> Option<&Arc<RwLock<Inventory>>> {
        self.inventories
            .iter()
            .find(|inv| inv.read().expect("Cannot read inventory uuid").uuid == inv_uuid)
    }

    // fn find_item(&mut self, inventory_uuid: Uuid, item_uuid: Uuid) -> Option<&Arc<RwLock<Item>>> {}
}

// #[macro_export]
// macro_rules! find_item {
//     ($inventory_uuid:ident,$item_uuid: ident) => {{
//         let inv = self
//             .inventories
//             .iter()
//             .find(|inv| inv.read().expect("Cannot read inventory uuid").uuid == inventory_uuid)
//             .expect("No such item")
//             .read()
//             .expect("Cannot read inventory");

//         inv.items
//             .iter()
//             .find(|item| item.read().expect("Cannot read item").uuid == item_uuid)
//     }};
// }
