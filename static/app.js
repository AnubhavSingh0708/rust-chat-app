// --- Initial Setup ---
if (document.cookie.includes("user_id")) {
    document.getElementById("loginModal").style.display = "none";
}

const userId = document.cookie.split('; ').find(row => row.startsWith('user_id='))?.split('=')[1];
const ws = new WebSocket(`ws://${window.location.host}/ws`);

// --- WebSocket Message Handler ---
ws.onmessage = async (event) => {
    const data = JSON.parse(event.data);

    // Render Online Users List (Memory state broadcast from Rust)
    if (data.type === "online_users") {
        const list = document.getElementById("onlineUsersList");
        list.innerHTML = "";
        data.users.forEach(u => {
            list.innerHTML += `
                <div class="online-user">
                    <img src="${u.pfp}" alt="pfp" onerror="this.src='/static/default.png'">
                    <span>${u.name}</span>
                </div>
            `;
        });
    }

    // Render New Chat Message
    if (data.type === "new_msg") {
        const msgDiv = document.createElement("div");
        msgDiv.className = "message-item";
        
        let attachmentHtml = "";
        if (data.attachment) {
            attachmentHtml = `<img src="${data.attachment}" class="message-attachment">`;
        }

        msgDiv.innerHTML = `
            <img src="${data.pfp}" class="message-avatar" onerror="this.src='/static/default.png'">
            <div class="message-content">
                <span class="message-header">${data.name}</span>
                <span>${data.content}</span>
                ${attachmentHtml}
            </div>
        `;
        const msgs = document.getElementById("messages");
        msgs.appendChild(msgDiv);
        msgs.scrollTop = msgs.scrollHeight; // Auto-scroll
    }

    // WebRTC Signaling
    if (data.type === "join_voice" && data.sender !== userId) {
        // Someone joined, create an offer
        const peer = createPeerConnection(data.sender);
        const offer = await peer.createOffer();
        await peer.setLocalDescription(offer);
        ws.send(JSON.stringify({ type: "offer", sender: userId, target: data.sender, sdp: offer }));
    }

    if (data.type === "offer" && data.target === userId) {
        const peer = createPeerConnection(data.sender);
        await peer.setRemoteDescription(new RTCSessionDescription(data.sdp));
        const answer = await peer.createAnswer();
        await peer.setLocalDescription(answer);
        ws.send(JSON.stringify({ type: "answer", sender: userId, target: data.sender, sdp: answer }));
    }

    if (data.type === "answer" && data.target === userId) {
        peers[data.sender].setRemoteDescription(new RTCSessionDescription(data.sdp));
    }

    if (data.type === "ice" && data.target === userId) {
        peers[data.sender].addIceCandidate(new RTCIceCandidate(data.candidate));
    }
};

function createPeerConnection(remoteUser) {
    const peer = new RTCPeerConnection({ iceServers: [{ urls: "stun:stun.l.google.com:19302" }] });
    peers[remoteUser] = peer;

    localStream.getTracks().forEach(track => peer.addTrack(track, localStream));

    peer.onicecandidate = (event) => {
        if (event.candidate) {
            ws.send(JSON.stringify({ type: "ice", sender: userId, target: remoteUser, candidate: event.candidate }));
        }
    };

    peer.ontrack = (event) => {
        setupRemoteAudio(remoteUser, event.streams[0]);
    };

    return peer;
}

// --- Audio Volume Management ---
function setupRemoteAudio(remoteUser, stream) {
    if (document.getElementById(`audio-${remoteUser}`)) return;

    // Web Audio API for volume control
    const source = audioCtx.createMediaStreamSource(stream);
    const gainNode = audioCtx.createGain();
    source.connect(gainNode);
    gainNode.connect(audioCtx.destination);

    // UI for Voice Panel
    const userDiv = document.createElement('div');
    userDiv.className = "voice-user";
    userDiv.id = `audio-${remoteUser}`;
    
    userDiv.innerHTML = `
        <span>User ${remoteUser.substring(0,4)} 🎤</span>
        <input type="range" min="0" max="2" step="0.1" value="1">
    `;
    
    // Adjust volume slider updates the GainNode
    userDiv.querySelector('input').addEventListener('input', (e) => {
        gainNode.gain.value = e.target.value;
    });

    document.getElementById('voiceUsers').appendChild(userDiv);
}

// --- Chat Form Submission ---
document.getElementById("chatForm").addEventListener("submit", async (e) => {
    e.preventDefault();
    const formData = new FormData();
    formData.append("content", document.getElementById("chatInput").value);
    
    const file = document.getElementById("attachment").files[0];
    if (file) formData.append("attachment", file);

    await fetch("/message", { method: "POST", body: formData });
    document.getElementById("chatInput").value = "";
    document.getElementById("attachment").value = "";
});