/// <reference types="vite/client" />
import React from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";

if (!import.meta.env.DEV) {
  document.addEventListener("contextmenu", (e) => e.preventDefault());
}

createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
