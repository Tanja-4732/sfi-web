use crate::{
    components::app::{AppRoute, AppRouterButton},
    services::data::{DataAgent, DataAgentRequest, DataAgentResponse},
};
use sfi_core::Item;
use yew::prelude::*;
use yew::{prelude::*, Bridge};

pub struct ItemCard {
    link: ComponentLink<Self>,
    props: Props,
}

pub enum Msg {
    OpenItem,
    EditItem,
}

#[derive(Clone, Properties)]
pub struct Props {
    pub item: Item,
}

impl Component for ItemCard {
    type Message = Msg;
    type Properties = Props;

    fn create(props: Self::Properties, link: ComponentLink<Self>) -> Self {
        Self { props, link }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        match msg {
            Msg::OpenItem => false,
            Msg::EditItem => {
                // TODO implement item edits
                true
            }
        }
    }

    fn change(&mut self, _props: Self::Properties) -> ShouldRender {
        false
    }

    fn view(&self) -> Html {
        if let Some(inventory) = self.props.item.inventory().upgrade() {
            let open_item_route = AppRoute::Units(*inventory.uuid(), *self.props.item.uuid());
            let update_item_route =
                AppRoute::UpdateItem(*inventory.uuid(), *self.props.item.uuid());

            html! {
                <div class="sfi-card">
                    <h3>{ self.props.item.name() }</h3>
                    <span class="sfi-subtitle">{ self.props.item.uuid() }</span>

                    <AppRouterButton route=open_item_route>{ "Open Item" }</AppRouterButton> { " " }
                    <AppRouterButton route=update_item_route>{ "Edit" }</AppRouterButton> { " " }
                </div>
            }
        } else {
            html! {
                <div class="sfi-card">
                    <h3>{ self.props.item.name() }</h3>
                    <span class="sfi-subtitle">{ "Couldn't find parent inventory" }</span>
                </div>
            }
        }
    }
}
