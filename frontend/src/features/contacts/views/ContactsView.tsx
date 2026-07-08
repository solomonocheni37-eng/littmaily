/**
 * Two-pane contact directory with client-side filtering.
 * Generates deterministic gradient avatars based on the contact's name hash
 * to maintain visual consistency across sessions.
 */
import { createSignal, createMemo, Show, For, createResource } from "solid-js";
import { ContactsApi } from "@/core/ipc";
import { useAppContext } from "@/core/store/AppStore";
import {
  Users,
  Search,
  Mail,
  Phone,
  MapPin,
  Building,
  Loader2,
} from "lucide-solid";
import { parseVCard } from "../utils/vcardParser";

const ContactsView = () => {
  const { state, openCompose } = useAppContext();

  const [search, setSearch] = createSignal("");
  const [selectedUid, setSelectedUid] = createSignal<string | null>(null);

  const [contacts] = createResource(
    () => state.selectedAccountId,
    async (accId) => (accId ? await ContactsApi.getAll(accId) : [])
  );

  const parsedContacts = createMemo(() => {
    const raw = contacts() || [];
    return raw
      .map((c) => ({
        raw: c,
        data: parseVCard(c.vcard_data),
      }))
      .sort((a, b) => a.data.fn.localeCompare(b.data.fn));
  });

  const filteredContacts = createMemo(() => {
    const q = search().toLowerCase().trim();
    if (!q) return parsedContacts();
    return parsedContacts().filter(
      (c) =>
        c.data.fn.toLowerCase().includes(q) ||
        c.data.emails.some((e) => e.toLowerCase().includes(q)) ||
        c.data.org.toLowerCase().includes(q)
    );
  });

  const selectedContact = createMemo(() => {
    const uid = selectedUid();
    if (!uid) return null;
    return (
      filteredContacts().find((c) => c.raw.uid === uid) ||
      parsedContacts().find((c) => c.raw.uid === uid)
    );
  });

  const getGradient = (name: string) => {
    const gradients = [
      "from-rose-400 to-orange-300",
      "from-indigo-400 to-purple-300",
      "from-emerald-400 to-teal-300",
      "from-sky-400 to-blue-300",
      "from-amber-400 to-yellow-300",
      "from-fuchsia-400 to-pink-300",
    ];
    let hash = 0;
    for (let i = 0; i < name.length; i++)
      hash = name.charCodeAt(i) + ((hash << 5) - hash);
    return gradients[Math.abs(hash) % gradients.length];
  };

  return (
    <div class="h-full flex bg-surface-50 dark:bg-surface-950 text-surface-900 dark:text-surface-50 overflow-hidden">
      {/* Left Pane: List */}
      <div class="w-80 flex-shrink-0 border-r border-surface-200 dark:border-surface-800 flex flex-col bg-white dark:bg-surface-900">
        <div class="p-4 border-b border-surface-200 dark:border-surface-800">
          <div class="relative">
            <Search
              size={16}
              class="absolute left-3 top-1/2 -translate-y-1/2 text-surface-400"
            />
            <input
              type="text"
              placeholder="Search contacts..."
              value={search()}
              onInput={(e) => setSearch(e.currentTarget.value)}
              class="w-full pl-9 pr-4 py-2 bg-surface-100 dark:bg-surface-800 rounded-lg border border-transparent focus:border-brand-500 text-sm outline-none transition-colors"
            />
          </div>
        </div>
        <div class="flex-1 overflow-y-auto">
          <Show
            when={!contacts.loading}
            fallback={
              <div class="p-8 text-center text-surface-500 flex flex-col items-center gap-2">
                <Loader2 class="animate-spin" /> Syncing...
              </div>
            }
          >
            <Show
              when={filteredContacts().length > 0}
              fallback={
                <div class="p-8 text-center text-surface-500 text-sm">
                  No contacts found.
                </div>
              }
            >
              <For each={filteredContacts()}>
                {(contact) => (
                  <button
                    onClick={() => setSelectedUid(contact.raw.uid)}
                    class={`w-full flex items-center gap-3 px-4 py-3 text-left transition-colors border-b border-surface-100 dark:border-surface-800/50 ${
                      selectedUid() === contact.raw.uid
                        ? "bg-brand-500/10 border-l-2 border-l-brand-500"
                        : "hover:bg-surface-50 dark:hover:bg-surface-800/50 border-l-2 border-l-transparent"
                    }`}
                  >
                    <div
                      class={`w-10 h-10 rounded-full bg-gradient-to-br ${getGradient(
                        contact.data.fn
                      )} flex items-center justify-center text-white text-sm font-semibold shadow-soft flex-shrink-0`}
                    >
                      {contact.data.fn.charAt(0).toUpperCase()}
                    </div>
                    <div class="min-w-0 flex-1">
                      <div class="font-medium text-sm truncate">
                        {contact.data.fn}
                      </div>
                      <div class="text-xs text-surface-500 truncate">
                        {contact.data.emails[0] ||
                          contact.data.org ||
                          "No details"}
                      </div>
                    </div>
                  </button>
                )}
              </For>
            </Show>
          </Show>
        </div>
      </div>

      {/* Right Pane: Details */}
      <div class="flex-1 overflow-y-auto p-8">
        <Show
          when={selectedContact()}
          fallback={
            <div class="h-full flex flex-col items-center justify-center text-surface-400">
              <Users size={48} class="mb-4 opacity-50" />
              <h3 class="text-lg font-medium">Select a contact</h3>
              <p class="text-sm">
                Choose a contact from the list to view their details.
              </p>
            </div>
          }
        >
          {(() => {
            const c = selectedContact()!;
            const d = c.data;
            return (
              <div class="max-w-2xl mx-auto">
                <div class="flex items-start gap-6 mb-8">
                  <div
                    class={`w-24 h-24 rounded-2xl bg-gradient-to-br ${getGradient(
                      d.fn
                    )} flex items-center justify-center text-white text-3xl font-bold shadow-elevated flex-shrink-0`}
                  >
                    {d.fn.charAt(0).toUpperCase()}
                  </div>
                  <div class="flex-1 min-w-0">
                    <h1 class="text-3xl font-bold mb-1 truncate">{d.fn}</h1>
                    <Show when={d.title}>
                      <div class="text-lg text-surface-600 dark:text-surface-400 truncate">
                        {d.title}
                      </div>
                    </Show>
                    <Show when={d.org}>
                      <div class="text-md text-surface-500 flex items-center gap-2 mt-1">
                        <Building size={14} /> {d.org}
                      </div>
                    </Show>
                  </div>
                </div>

                <div class="space-y-6">
                  <Show when={d.emails.length > 0}>
                    <div class="space-y-2">
                      <h3 class="text-xs font-bold uppercase tracking-wider text-surface-400 px-2">
                        Email
                      </h3>
                      <div class="bg-white dark:bg-surface-900 rounded-xl border border-surface-200 dark:border-surface-800 divide-y divide-surface-100 dark:divide-surface-800">
                        <For each={d.emails}>
                          {(email) => (
                            <button
                              onClick={() =>
                                openCompose({ type: "new", to: [email] })
                              }
                              class="w-full flex items-center gap-4 p-4 hover:bg-surface-50 dark:hover:bg-surface-800/50 transition-colors text-left"
                            >
                              <div class="w-10 h-10 rounded-lg bg-brand-500/10 flex items-center justify-center text-brand-500">
                                <Mail size={18} />
                              </div>
                              <div class="flex-1 min-w-0">
                                <div class="text-sm font-medium truncate">
                                  {email}
                                </div>
                                <div class="text-xs text-surface-500">
                                  Click to compose email
                                </div>
                              </div>
                            </button>
                          )}
                        </For>
                      </div>
                    </div>
                  </Show>

                  <Show when={d.tels.length > 0}>
                    <div class="space-y-2">
                      <h3 class="text-xs font-bold uppercase tracking-wider text-surface-400 px-2">
                        Phone
                      </h3>
                      <div class="bg-white dark:bg-surface-900 rounded-xl border border-surface-200 dark:border-surface-800 divide-y divide-surface-100 dark:divide-surface-800">
                        <For each={d.tels}>
                          {(tel) => (
                            <div class="flex items-center gap-4 p-4">
                              <div class="w-10 h-10 rounded-lg bg-emerald-500/10 flex items-center justify-center text-emerald-500">
                                <Phone size={18} />
                              </div>
                              <div class="text-sm font-medium">{tel}</div>
                            </div>
                          )}
                        </For>
                      </div>
                    </div>
                  </Show>

                  <Show when={d.adr.length > 0}>
                    <div class="space-y-2">
                      <h3 class="text-xs font-bold uppercase tracking-wider text-surface-400 px-2">
                        Address
                      </h3>
                      <div class="bg-white dark:bg-surface-900 rounded-xl border border-surface-200 dark:border-surface-800 divide-y divide-surface-100 dark:divide-surface-800">
                        <For each={d.adr}>
                          {(adr) => (
                            <div class="flex items-center gap-4 p-4">
                              <div class="w-10 h-10 rounded-lg bg-amber-500/10 flex items-center justify-center text-amber-500">
                                <MapPin size={18} />
                              </div>
                              <div class="text-sm">{adr}</div>
                            </div>
                          )}
                        </For>
                      </div>
                    </div>
                  </Show>
                </div>
              </div>
            );
          })()}
        </Show>
      </div>
    </div>
  );
};

export default ContactsView;
