// Temporary diagnostic: prints what the interface knows about the player's accounts.
const port = process.argv[2] || '9241';
const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = list.find((target) => target.type === 'page');
const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve) => (socket.onopen = resolve));

let nextId = 1;
const pending = new Map();
socket.onmessage = (event) => {
	const message = JSON.parse(event.data);
	if (pending.has(message.id)) pending.get(message.id)(message);
};

const evaluate = (expression) =>
	new Promise((resolve) => {
		const id = nextId++;
		pending.set(id, (message) => resolve(message.result?.result?.value));
		socket.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, awaitPromise: true, returnByValue: true } }));
	});

console.log('rows', await evaluate(`document.querySelectorAll('#play-accounts .account-row').length`));
console.log('store', JSON.stringify(await evaluate('window.aether.accounts.get()')));
socket.close();
