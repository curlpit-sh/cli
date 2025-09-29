import ReactDOM from "react-dom/client";

import { Router } from "./router";

const container = document.getElementById("root");

if (!container) {
  throw new Error("Root element not found");
}

ReactDOM.createRoot(container).render(<Router />);
