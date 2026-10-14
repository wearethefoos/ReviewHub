// ReviewHub content script: shows an "Open in ReviewHub" button on GitHub pull
// request pages. GitHub navigates without full page loads, so the URL is polled.
(() => {
  const PR_PATH = /^\/([\w.-]+)\/([\w.-]+)\/pull\/(\d+)/;
  const ID = "reviewhub-open-button";

  function current() {
    const match = PR_PATH.exec(location.pathname);
    return match ? `${match[1]}/${match[2]}#${match[3]}` : null;
  }

  function render() {
    const pr = current();
    let button = document.getElementById(ID);
    if (!pr) {
      button?.remove();
      return;
    }
    if (!button) {
      button = document.createElement("button");
      button.id = ID;
      button.type = "button";
      button.textContent = "Open in ReviewHub";
      button.title = "Review this pull request by what matters";
      Object.assign(button.style, {
        position: "fixed",
        right: "20px",
        bottom: "20px",
        zIndex: "2147483647",
        padding: "8px 14px",
        borderRadius: "999px",
        border: "1px solid rgba(240,246,252,0.1)",
        background: "#1f883d",
        color: "#fff",
        font: "600 13px -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
        boxShadow: "0 4px 12px rgba(0,0,0,0.25)",
        cursor: "pointer",
      });
      button.addEventListener("click", () => chrome.runtime.sendMessage({ type: "open-review", pr: current() }));
      document.body.appendChild(button);
    }
  }

  render();
  let last = location.href;
  setInterval(() => {
    if (location.href !== last) {
      last = location.href;
      render();
    }
  }, 750);
})();
