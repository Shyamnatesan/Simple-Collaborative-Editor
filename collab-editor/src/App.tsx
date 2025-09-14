import { useState } from "react";

function App() {
  const [roomId, setRoomId] = useState("");
  const [socket, setSocket] = useState<WebSocket | null>(null);
  const [text, setText] = useState("");

  const joinRoom = (roomId: string) => {
    const ws = new WebSocket(`ws://127.0.0.1:8080/ws/${roomId}`);

    ws.onopen = () => {
      setText(`You're in room ${roomId}`);
    };

    ws.onmessage = (event) => {
      // ✅ Update textarea when receiving updates
      if (typeof event.data === "string") {
        setText(event.data);
      }
    };

    ws.onerror = (err) => {
      console.error("WebSocket error:", err);
    };

    ws.onclose = () => {
      console.log("Disconnected from room:", roomId);
    };

    setSocket(ws);

    // maybe add logic to get old messages before joining
  };

  const createRoom = async () => {
    const res = await fetch("http://127.0.0.1:8080/create-room");
    const data = await res.json();
    console.log("Created room:", data.room_id);

    setRoomId(data.room_id); // keep roomId state in sync
    joinRoom(data.room_id);
  };

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newText = e.target.value;
    setText(newText);

    if (socket && socket.readyState === WebSocket.OPEN) {
      socket.send(newText);
    }
  };

  return (
    <div className="min-h-screen bg-gray-50 flex flex-col items-center py-10">
  <h1 className="text-3xl font-bold text-gray-800 mb-6">
    Editor
  </h1>

  {/* Controls */}
  <div className="flex items-center gap-3 mb-6">
    <input
      type="text"
      placeholder="Enter room ID"
      value={roomId}
      onChange={(e) => setRoomId(e.target.value)}
      className="px-4 py-2 border border-gray-300 rounded-lg focus:outline-none focus:ring-2 focus:ring-indigo-500 shadow-sm"
    />
    <button
      onClick={() => joinRoom(roomId)}
      className="px-4 py-2 bg-indigo-600 text-white rounded-lg shadow hover:bg-indigo-700 transition"
    >
      Join Room
    </button>
    <button
      onClick={createRoom}
      className="px-4 py-2 bg-green-600 text-white rounded-lg shadow hover:bg-green-700 transition"
    >
      Create Room
    </button>
    {socket && socket.readyState === WebSocket.OPEN && (
      <button
        onClick={() => {
          console.log("Exiting room:", roomId);
          socket.close(); // triggers Message::Close on Rust backend
          setSocket(null);
          setRoomId("");
          setText("");
        }}
        className="px-4 py-2 bg-red-600 text-white rounded-lg shadow hover:bg-red-700 transition"
      >
        Exit Room
      </button>
    )}
  </div>

  {/* Connection status */}
  {socket && socket.readyState === WebSocket.OPEN && (
    <p className="text-gray-700 mb-4">
      ✅ Connected to: <span className="font-semibold">{roomId}</span>
    </p>
  )}

  {/* Editor */}
  <textarea
    value={text}
    onChange={handleChange}
    rows={30}
    cols={60}
    className="p-4 border border-gray-300 rounded-lg shadow focus:outline-none focus:ring-2 focus:ring-indigo-500 resize-none font-mono"
  />
</div>

  );
}

export default App;
