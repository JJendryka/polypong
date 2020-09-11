import { config } from './config.js'

const socket = new WebSocket(config.websocket_server);

socket.addEventListener('open', function (event) {
    console.log('Connected to websocket server')
});

socket.addEventListener('message', function (event) {
    console.log('Message from server ', event.data);
});3