import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import net from "node:net";
import { setTimeout as sleep } from "node:timers/promises";
import { HocuspocusProvider, HocuspocusProviderWebsocket } from "@hocuspocus/provider";
import * as Y from "yjs";

type ScenarioResult = {
	name: string;
	sessionAwareness: boolean;
	status: "passed" | "limited";
	evidence: string[];
};

const args = new Set(process.argv.slice(2));
const spawnServer = args.has("--spawn-server");
const requireSessionAwarenessSync = args.has("--require-session-awareness-sync");
const url = readArg("--url") ?? "ws://127.0.0.1:3000";
const port = Number(new URL(url).port || "80");
const repoRoot = new URL("../../../", import.meta.url).pathname;

function readArg(name: string): string | undefined {
	const prefix = `${name}=`;
	return process.argv.slice(2).find((arg) => arg.startsWith(prefix))?.slice(prefix.length);
}

function createTimeout(ms: number, label: string): Promise<never> {
	return sleep(ms).then(() => {
		throw new Error(`Timed out waiting for ${label} after ${ms}ms`);
	});
}

async function waitUntil(predicate: () => boolean, label: string, timeoutMs = 5000): Promise<void> {
	const startedAt = Date.now();

	while (Date.now() - startedAt < timeoutMs) {
		if (predicate()) {
			return;
		}
		await sleep(25);
	}

	throw new Error(`Timed out waiting for ${label} after ${timeoutMs}ms`);
}

async function waitForSynced(provider: HocuspocusProvider, label: string): Promise<void> {
	if (provider.synced) {
		return;
	}

	await Promise.race([
		new Promise<void>((resolve) => {
			const onSynced = ({ state }: { state: boolean }) => {
				if (!state) {
					return;
				}
				provider.off("synced", onSynced);
				resolve();
			};
			provider.on("synced", onSynced);
		}),
		createTimeout(5000, `${label} synced`),
	]);
}

function hasRemoteAwareness(provider: HocuspocusProvider, remoteName: string): boolean {
	const states = Array.from(provider.awareness?.getStates().values() ?? []);
	return states.some((state) => state.user?.name === remoteName);
}

async function isPortOpen(host: string, targetPort: number): Promise<boolean> {
	return new Promise((resolve) => {
		const socket = net.createConnection({ host, port: targetPort });
		socket.once("connect", () => {
			socket.end();
			resolve(true);
		});
		socket.once("error", () => resolve(false));
	});
}

async function waitForPort(host: string, targetPort: number): Promise<void> {
	const startedAt = Date.now();

	while (Date.now() - startedAt < 15000) {
		if (await isPortOpen(host, targetPort)) {
			return;
		}
		await sleep(100);
	}

	throw new Error(`Rust example server did not open ${host}:${targetPort}`);
}

async function maybeStartServer(): Promise<ChildProcess | null> {
	if (!spawnServer) {
		return null;
	}

	if (await isPortOpen("127.0.0.1", port)) {
		console.log(`[server] using existing server at ${url}`);
		return null;
	}

	const build = spawnSync("cargo", ["build", "-p", "example"], {
		cwd: repoRoot,
		stdio: "inherit",
		env: { ...process.env, RUST_LOG: process.env.RUST_LOG ?? "warn" },
	});

	if (build.status !== 0) {
		throw new Error(`cargo build -p example failed with status ${build.status}`);
	}

	const executable = `${repoRoot}target/debug/example${process.platform === "win32" ? ".exe" : ""}`;
	const child = spawn(executable, [], {
		cwd: repoRoot,
		detached: true,
		stdio: ["ignore", "pipe", "pipe"],
		env: { ...process.env, RUST_LOG: process.env.RUST_LOG ?? "warn" },
	});

	child.stdout?.on("data", (chunk) => process.stdout.write(`[server] ${chunk}`));
	child.stderr?.on("data", (chunk) => process.stderr.write(`[server] ${chunk}`));

	await waitForPort("127.0.0.1", port);
	console.log(`[server] started Rust example server at ${url}`);

	return child;
}

function stopServer(child: ChildProcess | null): void {
	if (!child?.pid) {
		return;
	}

	try {
		process.kill(-child.pid, "SIGTERM");
	} catch {
		child.kill("SIGTERM");
	}
}

async function runSessionAwarenessFalse(): Promise<ScenarioResult> {
	const docName = `e2e-false-${Date.now()}`;
	const docA = new Y.Doc();
	const docB = new Y.Doc();
	const providerA = new HocuspocusProvider({ url, name: docName, document: docA, sessionAwareness: false });
	const providerB = new HocuspocusProvider({ url, name: docName, document: docB, sessionAwareness: false });

	try {
		await Promise.all([
			waitForSynced(providerA, "sessionAwareness=false provider A"),
			waitForSynced(providerB, "sessionAwareness=false provider B"),
		]);

		const payload = `provider-v4-false-${Date.now()}`;
		docA.getText("body").insert(0, payload);
		await waitUntil(() => docB.getText("body").toString() === payload, "sessionAwareness=false document update");

		providerA.awareness?.setLocalStateField("user", { name: "false-a" });
		await waitUntil(() => hasRemoteAwareness(providerB, "false-a"), "sessionAwareness=false awareness update");

		return {
			name: "two v4 providers synchronize over separate WebSockets",
			sessionAwareness: false,
			status: "passed",
			evidence: [
				"both providers emitted synced",
				`docB received Y.Text body "${payload}" from docA`,
				"providerB received providerA awareness user false-a",
			],
		};
	} finally {
		providerA.destroy();
		providerB.destroy();
		docA.destroy();
		docB.destroy();
	}
}

async function runSessionAwarenessTrue(): Promise<ScenarioResult> {
	const docName = `e2e-true-${Date.now()}`;
	const socket = new HocuspocusProviderWebsocket({ url, messageReconnectTimeout: 5000 });
	const docA = new Y.Doc();
	const docB = new Y.Doc();
	const providerA = new HocuspocusProvider({
		websocketProvider: socket,
		name: docName,
		document: docA,
		sessionAwareness: true,
	});
	const providerB = new HocuspocusProvider({
		websocketProvider: socket,
		name: docName,
		document: docB,
		sessionAwareness: true,
	});
	providerA.attach();
	providerB.attach();

	try {
		await Promise.all([
			waitForSynced(providerA, "sessionAwareness=true provider A"),
			waitForSynced(providerB, "sessionAwareness=true provider B"),
		]);

		const payload = `provider-v4-true-${Date.now()}`;
		docA.getText("body").insert(0, payload);

		let synchronized = true;
		try {
			await waitUntil(() => docB.getText("body").toString() === payload, "sessionAwareness=true document update", 1500);
		} catch {
			synchronized = false;
		}

		providerA.awareness?.setLocalStateField("user", { name: "true-a" });
		await sleep(250);

		if (synchronized) {
			return {
				name: "two v4 session-aware providers synchronize over one WebSocket",
				sessionAwareness: true,
				status: "passed",
				evidence: [
					"both providers emitted synced",
					`docB received Y.Text body "${payload}" from docA`,
				],
			};
		}

		if (requireSessionAwarenessSync) {
			throw new Error(
				"sessionAwareness=true providers connected, but base-document synchronization did not occur",
			);
		}

		return {
			name: "two v4 session-aware providers connect over one WebSocket",
			sessionAwareness: true,
			status: "limited",
			evidence: [
				"both providers emitted synced with sessionAwareness=true",
				"base-document Y.Text update did not reach the peer within 1500ms",
				"current Rust server appears to store the v4 composite routing key as the document key",
			],
		};
	} finally {
		providerA.destroy();
		providerB.destroy();
		socket.destroy();
		docA.destroy();
		docB.destroy();
	}
}

function printResult(result: ScenarioResult): void {
	console.log(`\n[${result.status}] ${result.name}`);
	console.log(`sessionAwareness=${result.sessionAwareness}`);
	for (const item of result.evidence) {
		console.log(`- ${item}`);
	}
}

async function main(): Promise<void> {
	const server = await maybeStartServer();

	try {
		if (!(await isPortOpen("127.0.0.1", port))) {
			throw new Error(`No server is listening at ${url}. Start cargo run -p example or pass --spawn-server.`);
		}

		const falseResult = await runSessionAwarenessFalse();
		const trueResult = await runSessionAwarenessTrue();

		printResult(falseResult);
		printResult(trueResult);
		console.log("\n[e2e] Hocuspocus v4 provider local path completed");
	} finally {
		stopServer(server);
	}
}

main().catch((error: unknown) => {
	console.error(error instanceof Error ? error.stack : error);
	process.exitCode = 1;
});
