// Evaluates an expression in the page of a running launcher, through the DevTools protocol.
//
// Usage: node cdp.mjs <port> "<expression>" [--no-await]
//
// `--no-await` is for calls that only start something: the answer is then the raw promise state,
// which the caller polls with another expression.

const port = process.argv[2] || '9333';
const expression = process.argv[3] || '1';
const awaitPromise = !process.argv.includes('--no-await');

const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = targets.find((target) => target.type === 'page' && target.webSocketDebuggerUrl);
if (!page) {
	console.error('no page on this port');
	process.exit(1);
}

const socket = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
	socket.onopen = resolve;
	socket.onerror = () => reject(new Error('the page refused the connection'));
});

const answer = await new Promise((resolve) => {
	socket.onmessage = (event) => resolve(JSON.parse(event.data));
	socket.send(
		JSON.stringify({
			id: 1,
			method: 'Runtime.evaluate',
			params: { expression, returnByValue: true, awaitPromise, userGesture: true }
		})
	);
});

socket.close();

if (answer.result?.exceptionDetails) {
	console.error(answer.result.exceptionDetails.text || 'the page threw');
	const exception = answer.result.exceptionDetails.exception;
	if (exception) console.error(exception.description || exception.value);
	process.exit(2);
}

const value = answer.result?.result?.value;
console.log(typeof value === 'string' ? value : JSON.stringify(value, null, 2));
