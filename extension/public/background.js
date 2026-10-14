// ReviewHub service worker: opens the review page for the pull request in the
// current tab (toolbar button) or the one the content script asks for.

const PR_PATH = /^https:\/\/github\.com\/([\w.-]+)\/([\w.-]+)\/pull\/(\d+)/;

function reviewUrl(pr) {
  return chrome.runtime.getURL(pr ? `review.html?pr=${encodeURIComponent(pr)}` : "review.html");
}

chrome.action.onClicked.addListener((tab) => {
  const match = PR_PATH.exec(tab.url ?? "");
  chrome.tabs.create({ url: reviewUrl(match ? `${match[1]}/${match[2]}#${match[3]}` : null), index: tab.index + 1 });
});

chrome.runtime.onMessage.addListener((message, sender) => {
  if (message?.type !== "open-review" || typeof message.pr !== "string") return;
  chrome.tabs.create({ url: reviewUrl(message.pr), index: (sender.tab?.index ?? 0) + 1 });
});
