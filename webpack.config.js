module.exports = {
    mode: "development",
    entry: {
        room: "./js/room.js",
    }, 
    output: {
        filename: "./js/[name].bundle.js" 
    },
    watch:true,
    resolve: { extensions: [".js", ".ts"] }
}