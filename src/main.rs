use std::collections::HashMap;

#[macro_use]
extern crate log;

use actix_files::NamedFile;
use actix_session::{CookieSession, Session as ActixSession};
use actix_web::{
    get, http, middleware::Logger, post, web, App, HttpResponse, HttpServer, Responder, Result,
};
use askama::Template;
use env_logger::Env;
use listenfd::ListenFd;
use log::Level;
use rand::{seq::SliceRandom, Rng};
use serde::Deserialize;
use tokio::sync::RwLock;

mod anonymous_names;
mod websockets;

static NICK_KEY: &str = "nick";
static ID_KEY: &str = "id";

#[derive(Template)]
#[template(path = "room.html")]
struct RoomTemplate<'a> {
    id: u64,
    users: Vec<&'a String>,
}

#[derive(Template)]
#[template(path = "joinRoom.html")]
struct JoinRoomTemplate {
    id: u64,
}

struct RoomState {
    rooms: RwLock<HashMap<u64, RwLock<Room>>>,
}

struct Room {
    users: HashMap<u64, User>,
    max_size: u64,
}

#[derive(Deserialize)]
struct CreateRoomForm {
    max_size: u64,
}

#[derive(Deserialize)]
struct JoinRoomForm {
    id: u64,
}

struct User {
    nick: String,
}

fn get_session(session: ActixSession) -> Result<(u64, User)> {
    if let (Some(_), Some(_)) = (
        session.get::<u64>(ID_KEY)?,
        session.get::<String>(NICK_KEY)?,
    ) {
    } else {
        let mut rng = rand::thread_rng();
        session.set(ID_KEY, rng.gen::<u64>())?;
        session.set(NICK_KEY, anonymous_names::ANONYMOUS_NAMES.choose(&mut rng))?;
    }
    Ok((
        session.get(ID_KEY)?.unwrap(),
        User {
            nick: session.get(NICK_KEY)?.unwrap(),
        },
    ))
}

#[get("/")]
async fn index() -> Result<NamedFile> {
    Ok(NamedFile::open("templates/index.html")?)
}

#[get("/rooms/new")]
async fn create_room_get() -> Result<NamedFile> {
    Ok(NamedFile::open("templates/createRoom.html")?)
}

#[post("/rooms/new")]
async fn create_room_post(
    form: web::Form<CreateRoomForm>,
    state: web::Data<RoomState>,
    session: ActixSession,
) -> Result<HttpResponse> {
    // Generate room id
    let mut id: u64 = rand::thread_rng().gen_range(0, 1000000);
    while let Some(_) = state.rooms.read().await.get(&id) {
        id = rand::thread_rng().gen_range(100000, 1000000);
    }

    // Create room
    let mut room = Room {
        max_size: form.max_size,
        users: HashMap::new(),
    };

    // Add current user to the room
    let (user_id, user) = get_session(session)?;
    room.users.insert(user_id, user);

    state.rooms.write().await.insert(id, RwLock::new(room));

    // Redirect to room page
    Ok(HttpResponse::SeeOther()
        .set_header(http::header::LOCATION, format!("/rooms/{}", id))
        .finish())
}

#[get("/rooms/{id}")]
async fn room_get(
    info: web::Path<u64>,
    state: web::Data<RoomState>,
    session: ActixSession,
) -> Result<HttpResponse> {
    let id = info.into_inner();
    let (user_id, _) = get_session(session)?;

    // Checking if room exists
    if let Some(room) = state.rooms.read().await.get(&id) {
        // Checking if user is inside room
        if let Some(_) = room.read().await.users.get(&user_id) {
            let s = RoomTemplate {
                id,
                users: room
                    .read()
                    .await
                    .users
                    .iter()
                    .map(|(_, user)| &user.nick)
                    .collect(),
            }
            .render()
            .unwrap();
            Ok(HttpResponse::Ok().content_type("text/html").body(s))
        } else {
            // User not inside room. Send to join page
            Ok(HttpResponse::SeeOther()
                .set_header(http::header::LOCATION, format!("/rooms/{}/join", id))
                .finish())
        }
    } else {
        // Room doesn't exist
        Ok(HttpResponse::NotFound().finish())
    }
}

#[post("/rooms/join")]
async fn room_join_form(
    form: web::Form<JoinRoomForm>,
    state: web::Data<RoomState>,
    session: ActixSession,
) -> Result<HttpResponse> {
    let (user_id, user) = get_session(session)?;
    let room_id = form.id;
    join_room(user_id, user, room_id, state).await
}

#[get("/rooms/{id}/join")]
async fn join_room_get(info: web::Path<u64>) -> Result<HttpResponse> {
    // TODO: Check if room exists and if user is in it
    let s = JoinRoomTemplate {
        id: info.into_inner(),
    }
    .render()
    .unwrap();
    Ok(HttpResponse::Ok().content_type("text/html").body(s))
}

#[post("/rooms/{id}/join")]
async fn join_room_post(
    info: web::Path<u64>,
    state: web::Data<RoomState>,
    session: ActixSession,
) -> Result<HttpResponse> {
    let (user_id, user) = get_session(session)?;
    let room_id = info.into_inner();
    join_room(user_id, user, room_id, state).await
}

async fn join_room(
    user_id: u64,
    user: User,
    room_id: u64,
    state: web::Data<RoomState>,
) -> Result<HttpResponse> {
    // Checking if room exists
    if let Some(room) = state.rooms.read().await.get(&room_id) {
        // Checking if user is already inside the room
        if let Some(_) = room.read().await.users.get(&user_id) {
            return Ok(HttpResponse::BadRequest().finish());
        }

        // Adding user to the room
        room.write().await.users.insert(user_id, user);

        // Redirecting to room page
        Ok(HttpResponse::SeeOther()
            .set_header(http::header::LOCATION, format!("/rooms/{}", room_id))
            .finish())
    } else {
        Ok(HttpResponse::NotFound().finish())
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    env_logger::from_env(Env::default().default_filter_or("info")).init();
    let mut listenfd = ListenFd::from_env();

    let room_state = web::Data::new(RoomState {
        rooms: RwLock::new(HashMap::new()),
    });

    let mut server = HttpServer::new(move || {
        App::new()
            .app_data(room_state.clone())
            .service(index)
            .service(create_room_get)
            .service(create_room_post)
            .service(room_get)
            .service(room_join_form)
            .service(join_room_get)
            .service(join_room_post)
            .service(actix_files::Files::new("/dist", "./dist").show_files_listing())
            .service(web::resource("/ws/").to(websockets::ws_connect_route))
            .wrap(Logger::default())
            // TODO: Change secret
            .wrap(CookieSession::signed(&[0; 32]).secure(false))
    });
    server = if let Some(l) = listenfd.take_tcp_listener(0).unwrap() {
        server.listen(l)?
    } else {
        server.bind("127.0.0.1:8088")?
    };

    server.run().await
}
