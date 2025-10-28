import * as Y from "yjs";
import { HocuspocusProvider } from "@hocuspocus/provider";

// Connect it to the backend
const provider = new HocuspocusProvider({
  url: "ws://127.0.0.1:4000/ws",
  name: "example-document1",
});

// Define `tasks` as an Array
const tasks = provider.document.getArray("tasks");

// Listen for changes
tasks.observe((event) => {
  console.log(event);
  console.log("tasks were modified");
});

// Add a new task
console.log("Adding new task");
tasks.push(["buy milk"]);