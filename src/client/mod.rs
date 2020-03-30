use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt;
use std::io::{self, Write};
use std::sync::{Arc, RwLock, Mutex};

use failure::Fail;
use anyhow::{Result, Context, Error};

use matrix_sdk::{
    self,
    api::r0::{
        directory::get_public_rooms_filtered,
        filter::RoomEventFilter,
        message::create_message_event,
        search::search_events::{self, Categories, Criteria},
        sync::sync_events,
    },
    events::{
        collections::all::{RoomEvent, StateEvent},
        room::aliases::AliasesEvent,
        room::canonical_alias::CanonicalAliasEvent,
        room::create::CreateEvent,
        room::member::{MemberEvent, MembershipState},
        room::message::{MessageEvent, MessageEventContent, TextMessageEventContent},
        room::name::{NameEvent, NameEventContent},
        EventResult, EventType,
    },
    identifiers::{UserId, RoomId, RoomAliasId},
    ruma_traits::{Endpoint, Outgoing},
    AsyncClient, AsyncClientConfig, Room, SyncSettings,
};
use url::Url;

mod event_stream;

#[derive(Clone, Debug)]
pub struct RoomInfo {
    pub name: Option<String>,
    pub alias: Option<RoomAliasId>,
    pub user: UserId,
}
impl RoomInfo {
    pub(crate) fn from_name(user: UserId, name: &str) -> Self {
        Self {
            name: Some(name.to_string()),
            user,
            alias: None,
        }
    }
    pub(crate) fn from_alias(user: UserId, alias: RoomAliasId) -> Self {
        Self {
            name: None,
            user,
            alias: Some(alias),
        }
    }
}
#[derive(Clone)]
pub struct MatrixClient {
    inner: AsyncClient,
    homeserver: String,
    pub current_room_id: Option<RoomId>,
    pub curr_sync: Option<String>,
    user: Option<UserId>,
}

impl fmt::Debug for MatrixClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MatrixClient")
            .field("user", &self.user)
            .finish()
    }
}

impl MatrixClient {
    pub fn new(homeserver: &str) -> Result<Self, failure::Error> {
        let client_config = AsyncClientConfig::default();
        let homeserver_url = Url::parse(&homeserver)?;

        let mut client = Self {
            inner: AsyncClient::new(homeserver_url, None)?,
            homeserver: homeserver.into(),
            user: None,
            current_room_id: None,
            curr_sync: None,
        };

        Ok(client)
    }

    pub(crate) async fn login(
        &mut self,
        username: String,
        password: String,
    ) -> Result<HashMap<String, Arc<RwLock<Room>>>> {

        let res = self.inner.login(username, password, None, None).await?;
        self.user = Some(res.user_id.clone());

        let response = self
            .inner
            .sync(SyncSettings::new().full_state(true))
            .await?;

        self.current_room_id = self.inner.current_room_id().await;
        Ok(self.inner.base_client().read().await.joined_rooms.clone())
    }

    pub(crate) async fn sync(
        &mut self,
        settings: matrix_sdk::SyncSettings,
        ee: Arc<Mutex<dyn matrix_sdk::EventEmitter>>,
    ) -> Result<()> {

        self.inner.sync_with(settings, ee).await.map_err(Error::from)
    }

    pub(crate) async fn send_message(
        &self,
        client: &mut AsyncClient,
        id: &str,
        msg: MessageEventContent,
    ) -> Result<create_message_event::Response> {
        client.room_send(&id, msg).await.context("Message failed to send")
    }
}
