use actix::{Actor, StreamHandler};
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;

struct RoomWS;

impl Actor for RoomWS {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("Connected");
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for RoomWS {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {}
}

pub async fn ws_connect_route(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    ws::start(RoomWS {}, &req, stream)
}
