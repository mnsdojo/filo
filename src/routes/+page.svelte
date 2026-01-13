<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";

  import { onMount } from "svelte";

  // --- Types ---
  interface Device {
    id: String;
    name: String;
    ip: String;
    port: number;
  }

  interface TransferProgress {
    filename: string;
    progress: number;
    speed: string;
  }

  // --- State ---
  let localIp = $state("Detecting...");
  let isServerRunning = $state(false);
  let devices = $state<Device[]>([]);
  let isScanning = $state(false);
  let transferProgress = $state<TransferProgress | null>(null);
  let statusMessage = $state("Ready");

  // --- Actions ---
  async function updateLocalIp() {
    try {
      localIp = await invoke("get_local_ip");
    } catch (e) {
      localIp = "Unknown";
      statusMessage = "Error detecting IP";
    }
  }

  async function startServer() {
    try {
      const result: string = await invoke("start_server");
      isServerRunning = true;
      statusMessage = result;
      await invoke("broadcast_presence");
    } catch (e) {
      statusMessage = "Failed to start server";
    }
  }

  async function scanDevices() {
    if (isScanning) return;
    isScanning = true;
    devices = [];
    statusMessage = "Scanning for devices...";
    try {
      devices = await invoke("discover_devices");
      statusMessage = `Found ${devices.length} devices`;
    } catch (e) {
      statusMessage = "Scan failed";
    } finally {
      isScanning = false;
    }
  }

  async function selectAndSendFile(device: Device) {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
      });

      if (selected && !Array.isArray(selected)) {
        statusMessage = `Sending file to ${device.name}...`;
        await invoke("send_file", {
          filepath: selected,
          targetIp: device.ip,
          targetPort: device.port,
        });
      }
    } catch (e) {
      statusMessage = `Error sending file: ${e}`;
    }
  }

  // --- Lifecycle & Listeners ---
  onMount(() => {
    updateLocalIp();

    const unlistenProgress = listen<TransferProgress>("transfer-progress", (event) => {
      transferProgress = event.payload;
    });

    const unlistenReceived = listen<string>("file-received", (event) => {
      statusMessage = `Received file: ${event.payload}`;
      transferProgress = null;
    });

    const unlistenSent = listen<string>("file-sent", (event) => {
      statusMessage = `Sent file: ${event.payload}`;
      transferProgress = null;
    });

    return () => {
      unlistenProgress.then((f) => f());
      unlistenReceived.then((f) => f());
      unlistenSent.then((f) => f());
    };
  });
</script>

<main class="app-container">
  <header>
    <div class="brand">
      <div class="logo-icon">F</div>
      <h1>Filo</h1>
    </div>
    <div class="status-chip" class:online={isServerRunning}>
      <span class="dot"></span>
      {isServerRunning ? "Server Online" : "Server Offline"}
    </div>
  </header>

  <section class="hero">
    <div class="glass-card main-info">
      <div class="info-group">
        <label>Your IP Address</label>
        <p class="ip-display">{localIp}</p>
      </div>
      <div class="actions">
        {#if !isServerRunning}
          <button class="btn-primary" onclick={startServer}>
            Start Receiving
          </button>
        {:else}
          <button class="btn-secondary" disabled>
            Receiving Files...
          </button>
        {/if}
      </div>
    </div>
  </section>

  <section class="discovery">
    <div class="section-header">
      <h2>Nearby Devices</h2>
      <button class="btn-icon" onclick={scanDevices} disabled={isScanning}>
        <span class:spinning={isScanning}>↻</span>
      </button>
    </div>

    <div class="device-list">
      {#each devices as device}
        <div class="device-card glass-card">
          <div class="device-info">
            <h3>{device.name}</h3>
            <p>{device.ip}</p>
          </div>
          <button class="btn-send" onclick={() => selectAndSendFile(device)}>
            Send File
          </button>
        </div>
      {:else}
        {#if !isScanning}
          <div class="empty-state">
            <p>No devices found. Click refresh to scan.</p>
          </div>
        {/if}
      {/each}
      
      {#if isScanning}
        <div class="skeleton-list">
           <div class="skeleton-card"></div>
           <div class="skeleton-card"></div>
        </div>
      {/if}
    </div>
  </section>


  {#if transferProgress}
    <div class="transfer-overlay">
      <div class="glass-card transfer-card">
        <h3>Transferring {transferProgress.filename}</h3>
        <div class="progress-bar-container">
          <div class="progress-bar" style="width: {transferProgress.progress}%"></div>
        </div>
        <div class="transfer-meta">
          <span>{transferProgress.progress.toFixed(1)}%</span>
          <span>{transferProgress.speed}</span>
        </div>
      </div>
    </div>
  {/if}

  <footer class="status-bar">
    <p>{statusMessage}</p>
  </footer>
</main>

<style>
  :root {
    --bg-color: #050505;
    --card-bg: rgba(255, 255, 255, 0.05);
    --accent-color: #3b82f6;
    --accent-glow: rgba(59, 130, 246, 0.5);
    --text-main: #ffffff;
    --text-muted: #a1a1aa;
    --border-color: rgba(255, 255, 255, 0.1);
    
    font-family: 'Inter', sans-serif;
    color: var(--text-main);
    background-color: var(--bg-color);
  }

  :global(body) {
    margin: 0;
    overflow: hidden;
    background: radial-gradient(circle at top right, #1e1e2e 0%, #050505 100%);
    height: 100vh;
  }

  .app-container {
    display: flex;
    flex-direction: column;
    height: 100vh;
    padding: 2rem;
    box-sizing: border-box;
    max-width: 900px;
    margin: 0 auto;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 3rem;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .logo-icon {
    width: 40px;
    height: 40px;
    background: linear-gradient(135deg, var(--accent-color), #8b5cf6);
    border-radius: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 1.25rem;
    box-shadow: 0 0 20px var(--accent-glow);
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0;
    letter-spacing: -0.025em;
  }

  .status-chip {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: var(--card-bg);
    padding: 0.5rem 1rem;
    border-radius: 99px;
    font-size: 0.875rem;
    border: 1px solid var(--border-color);
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #ef4444;
  }

  .status-chip.online .dot {
    background: #10b981;
    box-shadow: 0 0 8px #10b981;
  }

  .hero {
    margin-bottom: 3rem;
  }

  .glass-card {
    background: var(--card-bg);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid var(--border-color);
    border-radius: 24px;
    padding: 2rem;
  }

  .main-info {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: linear-gradient(110deg, rgba(255,255,255,0.05) 0%, rgba(255,255,255,0.02) 100%);
  }

  .info-group label {
    display: block;
    color: var(--text-muted);
    font-size: 0.875rem;
    margin-bottom: 0.5rem;
  }

  .ip-display {
    font-size: 2.5rem;
    font-weight: 700;
    margin: 0;
    background: linear-gradient(to right, #fff, #a1a1aa);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }

  .btn-primary {
    background: var(--accent-color);
    color: white;
    border: none;
    padding: 1rem 2rem;
    border-radius: 14px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
    box-shadow: 0 4px 15px rgba(59, 130, 246, 0.4);
  }

  .btn-primary:hover {
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(59, 130, 246, 0.6);
  }

  .btn-secondary {
    background: rgba(255,255,255,0.1);
    color: white;
    border: 1px solid var(--border-color);
    padding: 1rem 2rem;
    border-radius: 14px;
    font-weight: 600;
  }

  .discovery {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.5rem;
  }

  .section-header h2 {
    font-size: 1.125rem;
    font-weight: 600;
    color: var(--text-muted);
    margin: 0;
  }

  .btn-icon {
    background: var(--card-bg);
    border: 1px solid var(--border-color);
    color: white;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .device-list {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 1rem;
    overflow-y: auto;
    padding-bottom: 2rem;
  }

  .device-card {
    padding: 1.5rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    transition: transform 0.2s, border-color 0.2s;
  }

  .device-card:hover {
    transform: scale(1.02);
    border-color: var(--accent-color);
  }

  .device-info h3 {
    margin: 0 0 0.25rem 0;
    font-size: 1rem;
  }

  .device-info p {
    margin: 0;
    font-size: 0.875rem;
    color: var(--text-muted);
  }

  .btn-send {
    background: transparent;
    border: 1px solid var(--accent-color);
    color: var(--accent-color);
    padding: 0.5rem 1rem;
    border-radius: 8px;
    font-size: 0.875rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s;
  }

  .btn-send:hover {
    background: var(--accent-color);
    color: white;
  }

  .spinning {
    display: inline-block;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .transfer-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.8);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .transfer-card {
    width: 400px;
    text-align: center;
  }

  .progress-bar-container {
    height: 8px;
    background: rgba(255,255,255,0.1);
    border-radius: 4px;
    margin: 1.5rem 0 0.75rem 0;
    overflow: hidden;
  }

  .progress-bar {
    height: 100%;
    background: var(--accent-color);
    box-shadow: 0 0 10px var(--accent-glow);
    transition: width 0.3s ease;
  }

  .transfer-meta {
    display: flex;
    justify-content: space-between;
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  .status-bar {
    border-top: 1px solid var(--border-color);
    padding-top: 1rem;
    color: var(--text-muted);
    font-size: 0.875rem;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    grid-column: 1 / -1;
    text-align: center;
    padding: 4rem;
    min-height: 200px;
    color: var(--text-muted);
    border: 2px dashed var(--border-color);
    border-radius: 24px;
  }

  /* Skeleton Loader */
  .skeleton-card {
    height: 88px;
    background: var(--card-bg);
    border-radius: 24px;
    animation: pulse 1.5s infinite;
  }

  @keyframes pulse {
    0% { opacity: 0.5; }
    50% { opacity: 0.8; }
    100% { opacity: 0.5; }
  }
</style>

