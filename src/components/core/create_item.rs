use std::sync::{Arc, RwLock, RwLockReadGuard};

use sfi_core::core::Inventory;
use uuid::Uuid;
use yew::prelude::*;
use yew_router::{agent::RouteRequest, prelude::RouteAgentDispatcher};

use crate::{
    components::app::AppRoute,
    services::data::{DataAgent, DataAgentRequest, DataAgentResponse},
};

pub struct CreateItem {
    link: ComponentLink<Self>,
    name: String,
    inventory: Option<Arc<RwLock<Inventory>>>,
    inventory_uuid: Uuid,

    ean: Option<String>,
    data_bridge: Box<dyn Bridge<DataAgent>>,
    route_dispatcher: RouteAgentDispatcher,
    is_busy: bool,
}

pub enum Msg {
    UpdateName(String),
    UpdateEan(String),
    DataAgentResponse(DataAgentResponse),
    Confirm,
    Cancel,
}

#[derive(Clone, Properties)]
pub struct Props {
    pub inventory_uuid: Uuid,
}

impl Component for CreateItem {
    type Message = Msg;
    type Properties = Props;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        let inventory_uuid = props.inventory_uuid;

        let mut data_bridge = DataAgent::bridge(link.callback(Msg::DataAgentResponse));
        data_bridge.send(DataAgentRequest::GetInventory(inventory_uuid));

        Self {
            data_bridge,
            route_dispatcher: RouteAgentDispatcher::new(),
            name: String::new(),
            is_busy: false,
            ean: None,
            link,
            inventory: None,
            inventory_uuid,
        }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::UpdateName(name) => {
                self.name = name;
                false
            }
            Msg::UpdateEan(ean) => {
                self.ean = if ean.is_empty() { None } else { Some(ean) };
                true
            }
            Msg::Confirm => {
                // Give the new card to the listing component
                self.data_bridge.send(DataAgentRequest::CreateItem(
                    self.inventory_uuid,
                    self.name.clone(),
                    self.ean.clone(),
                ));

                self.is_busy = true;
                true
            }
            Msg::Cancel => {
                // Cancel the creation of the item
                self.route_dispatcher
                    .send(RouteRequest::ChangeRoute(AppRoute::Inventories.into()));

                self.is_busy = true;
                true
            }
            Msg::DataAgentResponse(response) => match response {
                DataAgentResponse::Inventory(inventory) => {
                    self.inventory = Some(inventory);
                    true
                }
                DataAgentResponse::InvalidInventoryUuid => {
                    self.inventory = None;
                    true
                }
                DataAgentResponse::NewItemUuid(_) => {
                    self.route_dispatcher.send(RouteRequest::ChangeRoute(
                        AppRoute::Items(self.inventory_uuid).into(),
                    ));

                    self.is_busy = false;
                    true
                }
                DataAgentResponse::Inventories(_)
                | DataAgentResponse::NewInventoryUuid(_)
                | DataAgentResponse::UpdatedItem
                | DataAgentResponse::Item(_)
                | DataAgentResponse::DeletedInventory(_)
                | DataAgentResponse::DeletedItem(_)
                | DataAgentResponse::UpdatedInventory(_) => false,
            },
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        let inventory = if let Some(inventory) = &self.inventory {
            inventory.read().expect("Cannot read inventory")
        } else {
            return html! { <p>{ "Cannot find this inventory" }</p> };
        };

        html! {
            <div>
                // A heading
                <h2>{ "Create a new item in " } {inventory.name.clone()}</h2>

                // The name input
                <input
                    type="text"
                    placeholder="name"
                    disabled=self.is_busy
                    value={self.name.to_owned()}
                    oninput=self.link.callback(|i: InputData| Msg::UpdateName(i.value))
                /> { " " }

                // The EAN input
                <input
                    type="text"
                    placeholder="EAN"
                    disabled=self.is_busy
                    value={self.ean.clone().unwrap_or(String::default())}
                    oninput=self.link.callback(|i: InputData| Msg::UpdateEan(i.value))
                /> { " " }

                // Save edits button
                <button
                    onclick=self.link.callback(|_| Msg::Confirm)
                    disabled=self.is_busy
                >
                    { "Save" }
                </button>  { " " }

                // Cancel button
                <button
                    onclick=self.link.callback(|_| Msg::Cancel)
                    disabled=self.is_busy
                >
                    { "Cancel" }
                </button>

            </div>
        }
    }
}
