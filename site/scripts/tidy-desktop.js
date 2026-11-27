// Run in the desktop app's page before a screenshot: show the test setup the way a studio would
// see it (its own folder, server address and people's names) instead of scratch paths.
const wsList = await (await fetch('http://127.0.0.1:18529/local/workspaces')).json();
const swaps = [
  ...wsList.map((w) => [w.root, 'D:\\Work\\' + w.project_name]),
  ['127.0.0.1:18560', 'mp.cinderworks.studio'],
  ['alice@example.com', 'alice@cinderworks.studio'],
];
const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
let node;
while ((node = walker.nextNode())) {
  let v = node.nodeValue;
  for (const [from, to] of swaps) v = v.split(from).join(to);
  if (v.trim() === 'alice') v = v.replace('alice', 'Alice Park');
  v = v.replace(/^alice ·/, 'Alice Park ·');
  if (v !== node.nodeValue) node.nodeValue = v;
}
