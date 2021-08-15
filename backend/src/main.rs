use futures::stream::StreamExt;
use nanoid::nanoid;
use serde::Serialize;
use std::{collections::HashMap, convert::Infallible, sync::Arc};
use tokio::sync::{
    mpsc::{self, UnboundedSender},
    RwLock,
};
use warp::{
    ws::{Message, WebSocket},
    Filter, Rejection, Reply,
};

type Wesult<T> = std::result::Result<T, Rejection>;

#[derive(Clone)]
struct Client {
    pos: usize,
    sender: UnboundedSender<Result<Message, warp::Error>>,
}

#[derive(Clone)]
pub struct Room {
    clients: Vec<Client>,
}

impl Room {
    fn new() -> Self {
        Self {
            clients: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct GameData {
    rooms: HashMap<String, Room>,
}

impl GameData {
    fn new() -> Self {
        Self {
            rooms: HashMap::new(),
        }
    }
}

type Game = Arc<RwLock<GameData>>;

fn with_game(game: Game) -> impl Filter<Extract = (Game,), Error = Infallible> + Clone {
    warp::any().map(move || game.clone())
}

#[derive(Serialize)]
struct CreateRoomResponse {
    id: String,
}

async fn create_room(game: Game) -> Wesult<warp::reply::Json> {
    let id = nanoid!();
    game.write().await.rooms.insert(id.clone(), Room::new());
    Ok(warp::reply::json(&CreateRoomResponse { id }))
}

async fn client_connection(ws: WebSocket, id: String, game: Game) {
    println!("Client connected to room {}", id);
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel::<Result<Message, warp::Error>>();
    let client = Client {
        pos: 0,
        sender: client_sender,
    };
    game.write()
        .await
        .rooms
        .get_mut(&id)
        .expect("Expected room to exist")
        .clients
        .push(client);
}

async fn room_join_handler(room_id: String, ws: warp::ws::Ws, game: Game) -> Wesult<impl Reply> {
    let r = game.read().await.rooms.get(&room_id).cloned();
    match r {
        Some(_) => Ok(ws.on_upgrade(move |socket| client_connection(socket, room_id, game))),
        None => Err(warp::reject::not_found()),
    }
}

#[tokio::main]
async fn main() {
    let game = Arc::new(RwLock::new(GameData::new()));

    let index = warp::path::end().map(|| "ferristype server v0.1.0");

    let room_create_route = warp::path!("rooms" / "create")
        .and(warp::post())
        .and(with_game(game.clone()))
        .and_then(create_room);

    let room_join_route = warp::path!("room" / String / "join")
        .and(warp::ws())
        .and(with_game(game))
        .and_then(room_join_handler);

    let routes = index.or(room_create_route).or(room_join_route);

    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}
