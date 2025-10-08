<script lang="ts">
    // import BubbleMenu from ';
    import { onMount, onDestroy } from "svelte";
    import { Editor } from "@tiptap/core";
    import { StarterKit } from "@tiptap/starter-kit";
    import * as Y from "yjs";
    import Collaboration from "@tiptap/extension-collaboration";
    import { HocuspocusProvider } from "@hocuspocus/provider";
    import CollaborationCaret from "@tiptap/extension-collaboration-caret";
    // import BubbleMenu from "@tiptap/extension-bubble-menu";

    let bubbleMenu: HTMLDivElement | null = $state(null);
    let element: HTMLElement | null = $state(null);
    let editorState: { editor: Editor | null } = $state({ editor: null });

    // export let port: number = 3000;
    let { port, name } = $props();

    const room = `room.${new Date()
        .getFullYear()
        .toString()
        .slice(-2)}${new Date().getMonth() + 1}${new Date().getDate()}-ok`;

    onMount(() => {
        const ydocA = new Y.Doc();
        const provider = new HocuspocusProvider({
            name: room,
            document: ydocA,
            url: `ws://127.0.0.1:${port}`,
        });
        // provider.document.off("update", provider.boundDocumentUpdateHandler);
        // let updateBuffer: Uint8Array[] = [];
        // let flushTimeout = null;
        // provider.boundDocumentUpdateHandler = function (update, origin) {
        //     // Provider 전송을 막고 버퍼에 저장
        //     updateBuffer.push(update);

        //     clearTimeout(flushTimeout);
        //     flushTimeout = setTimeout(() => {
        //         if (updateBuffer.length === 0) return;

        //         // 모든 업데이트를 머지
        //         const mergedUpdate = Y.mergeUpdates(updateBuffer);

        //         // 한번에 브로드캐스트
        //         provider.documentUpdateHandler(mergedUpdate, origin);

        //         updateBuffer = [];
        //     }, 300); // 300ms 동안 모은 후 전송
        // };
        // provider.document.on("update", provider.boundDocumentUpdateHandler);

        // provider.document.off("update", provider.boundDocumentUpdateHandler);
        // let updateBuffer2: Uint8Array[] = [];
        // let flushTimeout2 = null;
        // provider.boundDocumentUpdateHandler = function (update, origin) {
        //     // Provider 전송을 막고 버퍼에 저장
        //     updateBuffer2.push(update);

        //     clearTimeout(flushTimeout2);
        //     flushTimeout2 = setTimeout(() => {
        //         if (updateBuffer2.length === 0) return;

        //         // 모든 업데이트를 머지
        //         const mergedUpdate = Y.mergeUpdates(updateBuffer2);

        //         // 한번에 브로드캐스트
        //         provider.documentUpdateHandler(mergedUpdate, origin);

        //         updateBuffer2 = [];
        //     }, 300); // 300ms 동안 모은 후 전송
        // };
        // provider.document.on("update", provider.boundDocumentUpdateHandler);

        // end awareness example

        if (!element || !bubbleMenu) {
            return;
        }

        const editor = new Editor({
            element,
            extensions: [
                StarterKit,
                Collaboration.configure({
                    document: ydocA,
                }),
                CollaborationCaret.configure({
                    provider,
                    user: {
                        name: "Cyndi Lauper",
                        color: "#f783ac",
                    },
                }),
                // BubbleMenu.configure({
                //     element: bubbleMenu,
                // }),
            ],
            content: "",
            onTransaction: ({ editor }) => {
                // force re-render so `editor.isActive` works as expected
                editorState = { editor };
            },
        });

        editorState = { editor };
    });

    onDestroy(() => {
        editorState.editor?.destroy();
    });
</script>

<div style="position: relative" class="app">
    <h2>{name}</h2>
    <div class="bubble-menu" bind:this={bubbleMenu}>
        {#if editorState.editor}
            {@const editor = editorState.editor!}
            <button
                onclick={() => editor.chain().focus().toggleBold().run()}
                class:active={editor.isActive("bold")}
            >
                Bold
            </button>
            <button
                onclick={() => editor.chain().focus().toggleItalic().run()}
                class:active={editor.isActive("italic")}
            >
                Italic
            </button>
            <button
                onclick={() => editor.chain().focus().toggleStrike().run()}
                class:active={editor.isActive("strike")}
            >
                Strike
            </button>
        {/if}
    </div>

    {#if editorState.editor}
        {@const editor = editorState.editor!}
        <div class="fixed-menu">
            <button
                onclick={() =>
                    editor.chain().focus().toggleHeading({ level: 1 }).run()}
                class:active={editor.isActive("heading", {
                    level: 1,
                })}
            >
                H1
            </button>
            <button
                onclick={() =>
                    editor.chain().focus().toggleHeading({ level: 2 }).run()}
                class:active={editor.isActive("heading", {
                    level: 2,
                })}
            >
                H2
            </button>
            <button
                onclick={() => editor.chain().focus().setParagraph().run()}
                class:active={editor.isActive("paragraph")}
            >
                P
            </button>
        </div>
    {/if}

    <div class="pray-editor" bind:this={element}></div>
</div>

<style>
    .bubble-menu {
        display: flex;
        gap: 0.5rem;
        padding: 0.25rem 0.5rem;
        border-radius: 0.5rem;
        background: white;
        border: 1px solid #d4d4d8;
        box-shadow: 0 4px 12px hsla(0, 0%, 0%, 0.15);
    }
    .pray-editor {
        border: 1px solid #d4d4d8;
        border-radius: 0.5rem;
        padding: 1rem;
        min-height: 300px;
        margin-top: 2rem;
    }

    button.active {
        background: black;
        color: white;
    }

    :global(.collaboration-carets__caret) {
        border-left: 1px solid #0d0d0d;
        border-right: 1px solid #0d0d0d;
        margin-left: -1px;
        margin-right: -1px;
        pointer-events: none;
        position: relative;
        word-break: normal;
    }

    /* Render the username above the caret */
    :global(.collaboration-carets__label) {
        border-radius: 3px 3px 3px 0;
        color: #0d0d0d;
        font-size: 12px;
        font-style: normal;
        font-weight: 600;
        left: -1px;
        line-height: normal;
        padding: 0.1rem 0.3rem;
        position: absolute;
        top: -1.4em;
        user-select: none;
        white-space: nowrap;
    }
    .tiptap {
        :first-child {
            margin-top: 0;
        }

        /* Placeholder (at the top) */
        p.is-editor-empty:first-child::before {
            color: var(--gray-4);
            content: attr(data-placeholder);
            float: left;
            height: 0;
            pointer-events: none;
        }

        p {
            word-break: break-all;
        }

        /* Give a remote user a caret */
        .collaboration-carets__caret {
            border-left: 1px solid #0d0d0d;
            border-right: 1px solid #0d0d0d;
            margin-left: -1px;
            margin-right: -1px;
            pointer-events: none;
            position: relative;
            word-break: normal;
        }

        /* Render the username above the caret */
        .collaboration-carets__label {
            border-radius: 3px 3px 3px 0;
            color: #0d0d0d;
            font-size: 12px;
            font-style: normal;
            font-weight: 600;
            left: -1px;
            line-height: normal;
            padding: 0.1rem 0.3rem;
            position: absolute;
            top: -1.4em;
            user-select: none;
            white-space: nowrap;
        }
    }
</style>
