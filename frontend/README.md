# Frontend Visualizer

This directory contains the Svelte 5 and D3.js frontend for the Decentralized Telemetry Gossip Network.

## Setup

1.  **Install Dependencies:**
    Navigate to this `frontend/` directory and install the necessary Node.js packages.
    ```sh
    cd frontend
    npm install
    ```

## Development

To run the frontend in development mode with live reloading, you must first have the backend cluster running via the `orchestrator.sh` script.

1.  **Start the Backend Cluster:**
    From the project root, run the orchestrator.
    ```sh
    # Example: 20 nodes, 4 communities
    ./orchestrator.sh 20 4 0.8 0.05
    ```
    This will start the designated visualizer node on `http://127.0.0.1:8080`.

2.  **Start the Frontend Dev Server:**
    In a separate terminal, from the `frontend/` directory, run the Vite development server.
    ```sh
    npm run dev
    ```
    The Vite server will start on `http://localhost:5173` and automatically proxy WebSocket requests to the backend. Open this URL in your browser.

## Build

To create a production build for deployment:

```sh
npm run build