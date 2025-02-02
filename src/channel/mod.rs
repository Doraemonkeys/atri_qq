use std::sync::OnceLock;

use async_trait::async_trait;
use regex::Regex;
use ricq::handler::QEvent;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tracing::info;

use crate::{Bot, get_app, get_listener_runtime};
use crate::event::{BotOnlineEvent, Event, EventInner, GroupMessageEvent};
use crate::service::listeners::get_global_worker;

static GLOBAL_EVENT_CHANNEL: OnceLock<Sender<Event>> = OnceLock::<Sender<Event>>::new();

pub fn global_sender() -> &'static Sender<Event> {
    GLOBAL_EVENT_CHANNEL.get_or_init(|| {
        let channel = channel(128);

        channel.0
    })
}

pub fn global_receiver() -> Receiver<Event> {
    global_sender().subscribe()
}

pub struct GlobalEventBroadcastHandler;

#[async_trait]
impl ricq::handler::Handler for GlobalEventBroadcastHandler {
    async fn handle(&self, event: QEvent) {
        let bot_id: i64;
        let bot: Bot;

        let _event_: Event;
        fn get_bot(id: i64) -> Bot {
            get_app().bots.get(&id).expect("Cannot find bot").clone()
        }

        match event {
            QEvent::Login(id) => {
                bot_id = id;
                bot = get_bot(bot_id);

                let base = BotOnlineEvent::from(bot);
                let inner = Event::BotOnlineEvent(base);
                _event_ = inner.into();
            }
            QEvent::GroupMessage(e) => {
                bot_id = e.client.uin().await;
                if bot_id == e.inner.from_uin { return; }
                bot = get_bot(bot_id);

                let group = if let Some(g) = bot.find_group(e.inner.group_code) {
                    g
                } else { return; };

                let filter = get_filter_regex();

                info!("{group}{0} to {bot}: {1}",
                    filter.replace_all(group.name(), ""),
                    e.inner.elements,
                );

                let base = GroupMessageEvent::from(
                    group,
                    e,
                );
                _event_ = Event::GroupMessageEvent(base);
            }
            or => {
                _event_ = Event::Unknown(EventInner::<QEvent>::from(or));
            }
        }

        let e = _event_.clone();
        get_listener_runtime().spawn(async move {
            get_global_worker().handle(&e).await;
        });

        let _ = global_sender().send(_event_);
    }
}

static FILTER_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_filter_regex() -> &'static Regex {
    FILTER_REGEX.get_or_init(|| {
        Regex::new("<[$&].+>").expect("Cannot parse regex")
    })
}