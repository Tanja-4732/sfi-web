use anyhow::Result;
use serde::{Deserialize, Serialize};
use sfi_core::users::{StatusNotice, UserInfo, UserLogin, UserSignup};
use std::{collections::HashSet, rc::Rc};
use yew::{
    format::{Json, Nothing},
    services::{
        fetch::{FetchOptions, Request as FetchRequest, Response as FetchResponse},
        FetchService,
    },
    web_sys::RequestCredentials,
    worker::*,
};

use crate::components::login::AuthState;

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
    GetAuthStatus,
    Login(UserLogin),
    Signup(UserSignup),
    Logout,
}

pub enum Msg {
    LoggedIn(UserInfo),
    LoggedOut,
    LoginError(anyhow::Error),
}

pub struct AuthAgent {
    link: AgentLink<AuthAgent>,
    subscribers: HashSet<HandlerId>,
}

impl Agent for AuthAgent {
    type Reach = Context<Self>;
    type Message = Msg;
    type Input = Request;
    type Output = Rc<AuthState>;

    fn create(link: AgentLink<Self>) -> Self {
        Self {
            link,
            subscribers: HashSet::new(),
        }
    }

    fn update(&mut self, msg: Self::Message) {
        // Inform subscribers about internal changes from fetch callbacks
        let output = Rc::new(match msg {
            Msg::LoggedIn(user_info) => AuthState::LoggedIn(user_info),
            Msg::LoginError(error) => AuthState::Error(error),
            Msg::LoggedOut => AuthState::Initial,
        });

        for sub in self.subscribers.iter() {
            self.link.respond(*sub, output.clone());
        }
    }

    fn handle_input(&mut self, msg: Self::Input, _id: HandlerId) {
        // Handle authentication  requests from components and other agents
        match msg {
            Request::GetAuthStatus => {
                log::debug!("Getting auth status");
                let output = Rc::new(self.probe_state());

                for sub in self.subscribers.iter() {
                    self.link.respond(*sub, output.clone());
                }
            }
            Request::Login(login_info) => {
                log::debug!("Logging in");
                let output = Rc::new(self.login(login_info));

                for sub in self.subscribers.iter() {
                    self.link.respond(*sub, output.clone());
                }
            }
            Request::Signup(signup_info) => {
                log::debug!("Signing up");
                let output = Rc::new(self.signup(signup_info));

                for sub in self.subscribers.iter() {
                    self.link.respond(*sub, output.clone());
                }
            }
            Request::Logout => {
                log::debug!("Logging out");
                let output = Rc::new(self.logout());

                for sub in self.subscribers.iter() {
                    self.link.respond(*sub, output.clone());
                }
            }
        }
    }

    fn connected(&mut self, id: HandlerId) {
        self.subscribers.insert(id);
    }

    fn disconnected(&mut self, id: HandlerId) {
        self.subscribers.remove(&id);
    }
}

impl AuthAgent {
    fn login(&mut self, login_info: UserLogin) -> AuthState {
        let request = FetchRequest::post("http://localhost:8080/api/v1/authentication/login")
            .header("Content-Type", "application/json")
            .body(Json(&login_info))
            .expect("Failed to build request (login).");

        let options = FetchOptions {
            credentials: Some(RequestCredentials::SameOrigin),
            ..FetchOptions::default()
        };

        let callback = self
            .link
            .callback(|response: FetchResponse<Json<Result<UserInfo>>>| {
                let Json(data) = response.into_body();

                match data {
                    Ok(user) => Msg::LoggedIn(user),
                    Err(error) => Msg::LoginError(error),
                }
            });

        let task = FetchService::fetch_with_options(request, options, callback);

        // Store the task so it isn't canceled immediately
        match task {
            Ok(fetch_task) => AuthState::LoggingIn(fetch_task),
            Err(error) => AuthState::Error(error),
        }
    }

    fn signup(&mut self, signup_info: UserSignup) -> AuthState {
        let request = FetchRequest::post("http://localhost:8080/api/v1/authentication/signup")
            .header("Content-Type", "application/json")
            .body(Json(&signup_info))
            .expect("Failed to build request (signup).");

        let options = FetchOptions {
            credentials: Some(RequestCredentials::SameOrigin),
            ..FetchOptions::default()
        };

        let callback = self
            .link
            .callback(|response: FetchResponse<Json<Result<UserInfo>>>| {
                let Json(data) = response.into_body();

                match data {
                    Ok(user) => Msg::LoggedIn(user),
                    Err(error) => Msg::LoginError(error),
                }
            });

        let task = FetchService::fetch_with_options(request, options, callback);

        // Store the task so it isn't canceled immediately
        match task {
            Ok(fetch_task) => AuthState::LoggingIn(fetch_task),
            Err(error) => AuthState::Error(error),
        }
    }

    fn logout(&mut self) -> AuthState {
        let request = FetchRequest::get("http://localhost:8080/api/v1/authentication/logout")
            .body(Nothing)
            .expect("Failed to build request (logout).");

        let options = FetchOptions {
            credentials: Some(RequestCredentials::SameOrigin),
            ..FetchOptions::default()
        };

        let callback = self
            .link
            .callback(|response: FetchResponse<Json<Result<StatusNotice>>>| {
                let Json(data) = response.into_body();

                match data {
                    Ok(_) => Msg::LoggedOut,
                    Err(error) => Msg::LoginError(error),
                }
            });

        let task = FetchService::fetch_with_options(request, options, callback);

        // Store the task so it isn't canceled immediately
        match task {
            Ok(fetch_task) => AuthState::LoggingOut(fetch_task),
            Err(error) => AuthState::Error(error),
        }
    }

    fn probe_state(&self) -> AuthState {
        let request = FetchRequest::get("http://localhost:8080/api/v1/authentication/status")
            .body(Nothing)
            .expect("Failed to build request (probe).");

        let options = FetchOptions {
            credentials: Some(RequestCredentials::SameOrigin),
            ..FetchOptions::default()
        };

        let callback = self
            .link
            .callback(|response: FetchResponse<Json<Result<UserInfo>>>| {
                let Json(data) = response.into_body();

                match data {
                    Ok(user) => Msg::LoggedIn(user),
                    Err(_) => Msg::LoggedOut,
                }
            });

        let task = FetchService::fetch_with_options(request, options, callback);

        // Store the task so it isn't canceled immediately
        match task {
            Ok(fetch_task) => AuthState::Probing(fetch_task),
            Err(error) => AuthState::Error(error),
        }
    }
}
