# Translating Fogata

Fogata's own words (everything the window says: pages, buttons, tooltips, menus, the tray
menu, notifications, errors, screen-reader labels) come from ARB files in `app/lib/l10n/`,
made into Dart by Flutter's `gen-l10n` (`app/l10n.yaml`; `flutter pub get` and every build
run it, and `flutter gen-l10n` runs it by hand). What people write (names, messages, file
names) is never translated, and the command-line program stays in English.

| Language | File | Locale |
|---|---|---|
| English (the source) | `app_en.arb` | `en` |
| Español | `app_es.arb` | `es` |
| Português do Brasil | `app_pt.arb` | `pt` (`pt-BR` in Settings) |
| Français | `app_fr.arb` | `fr` |
| Deutsch | `app_de.arb` | `de` |
| Italiano | `app_it.arb` | `it` |
| Русский | `app_ru.arb` | `ru` |
| 日本語 | `app_ja.arb` | `ja` |
| 简体中文 | `app_zh.arb` | `zh` (`zh-Hans` in Settings) |

`app_pt.arb` holds Brazilian Portuguese and `app_zh.arb` Simplified Chinese: gen-l10n needs
the plain language as the fallback of a regional one, so a European Portuguese or a
Traditional Chinese would come as `app_pt_PT.arb` or `app_zh_Hant.arb` with only what
differs.

## How the app uses them

`lib/src/l10n.dart` keeps the language in use: `l10n` (the generated `L10n` of that
language), `currentLanguage`, the list `languages` (each named in itself) and
`useLanguage`. Code reads `l10n.someMessage` or `l10n.someMessage(args)`, in widgets and in
state alike; when the language changes in Settings the whole window is built again
(`FogataApp` marks every element to build, keeping their state), and the tray menu is set
again. The first time, the language is the system's first one that Fogata has (any
Portuguese is Brazilian; Chinese in simplified characters, or from China or Singapore, is
Simplified Chinese; Traditional Chinese is not offered yet), else English; once chosen it is
kept as `uiLanguage` in `app.json` (versions that offered only English wrote `language`,
which is not read: it was never a choice).

Dates, times and sizes follow the language through `intl` (`format.dart`): `clock` keeps the
12- or 24-hour choice of Settings (`午後8:13` in Japanese and Chinese), `dayClock`/`day` write
the day the language's way (`21 oct.`, `21. Okt.`, `10月21日`), `bytes` uses its decimal
mark and units (`48,2 Mo`), and `ago`, `timeLeft`, `spanWords`, `names` and `ordinal` are
messages too. The core's errors arrive in English and `explain` finds the known ones by a
part of their words (kept as the core writes them) to say them in the language in use; a
move's failures and code problems come from the core as kinds (`UiMoveReason`,
`UiCodeProblem`), and only system error text and what the other computer said in its own
words come as they are.

Japanese and Chinese are drawn with the system's fonts (the interface font has neither):
`cjkFonts` in `theme.dart` lists them after the emoji font, the language in use first so
the same character takes its own form in each (Hiragino Sans and PingFang SC on macOS, Yu
Gothic UI, Meiryo and Microsoft YaHei UI on Windows, Noto Sans CJK JP/SC, Source Han Sans
and WenQuanYi Micro Hei on Linux). A Linux without any of them shows those characters with
whatever fontconfig finds, or as boxes.

## Adding a message

1. Write it in `app_en.arb` with a key that starts with its file's name in camelCase
   (`roomLeave` for `screens/room.dart`) and an `@key` entry whose `description` says where
   it shows, what it is (button, tooltip, title, toast, screen-reader label), how much
   room it has, and what each placeholder holds. Every key needs one
   (`required-resource-attributes`).
2. One message per sentence: never glue messages together, never build a sentence from
   pieces in code. Words that change with a value go inside the message: placeholders
   (`{name}`), counts as ICU plurals (`{n, plural, =1{1 member} other{{n} members}}`; gen-l10n
   takes the cases `=0`, `=1`, `=2`, `zero`, `one`, `two`, `few`, `many` and `other`) and
   choices as ICU selects (`{system, select, windows{…} macos{…} other{…}}`). A part drawn
   its own way (bold, the digits' font) is a placeholder the code fills with a mark and
   draws apart (`styledSpans` in `typing.dart`).
   gen-l10n treats `=1` as `one`, and says the `one` case for every number of that
   category: in French and Portuguese 0 too, in Russian 21, 31…; there that case shows the
   number (`one{{n} файл}`), or 0 gets an `=0` of its own. Never both `=1` and `one`.
3. Add it to every other `app_*.arb` (the tests fail without it; a message missing in a
   language would show English there), run `flutter gen-l10n`, and use it as
   `l10n.yourKey`.
4. `flutter test test/l10n_test.dart test/languages_fit_test.dart`.

## Adding a language

1. Add `app_<code>.arb` with `"@@locale": "<code>"` and every message, then its code and
   its own name to `languages` in `lib/src/l10n.dart` (and to `localeOf`/`intlLocale` if
   its code is not plain), and its plural categories to `test/l10n_test.dart`.
2. Write its part of the glossary below first, and keep to it.
3. If it needs fonts of the system (scripts Inter does not cover), add them to `theme.dart`.
4. Add it to `CFBundleLocalizations` in `macos/Runner/Info.plist`, so macOS shows its own
   dialogs (file choosers, permission prompts' buttons) in it, and write the permission
   prompts' texts (each `NS…UsageDescription` of Info.plist) in
   `macos/Runner/<code>.lproj/InfoPlist.strings` (`pt-BR`, `zh-Hans`: the codes Info.plist
   uses), added to the Runner target's resources in Xcode (the `InfoPlist.strings` group:
   a new localization of it). `test/macos_permission_texts_test.dart` fails until both are
   done, and when a new usage key misses a language.
5. Check it: `flutter test test/l10n_test.dart test/languages_fit_test.dart`, then the
   screens with `FOGATA_TEST_LANGUAGE=<code>`, at the default size and with
   `FOGATA_TEST_WINDOW=960x640` (see `integration_test/screens_test.dart`), and look at
   every picture.

## Checks

- `test/l10n_test.dart`: every language has every message of English, with the same
  placeholders and select cases, plural categories the language has (Russian always
  `one`, `few` and `many`, and the pitfalls above), and ICU that reads; a few formats per
  language.
- `test/languages_fit_test.dart`: the main pages (room, home, a direct chat, friends,
  members, the person's panel, the bell, every part of the settings and the report's
  list and its last step, every tab of a group's settings, the title bar's update pill, every
  step of the splash in its 300 by 360 window) in every language
  and in a pseudo-language that makes every English word 40 % longer, at 960 by 640 and
  with text at 1 and 1.5 times its size: nothing overflows.
- `integration_test/screens_test.dart` with `FOGATA_TEST_LANGUAGE` (and
  `FOGATA_TEST_WINDOW`): pictures of every screen of a real copy in that language.

Not checked by tests: Japanese and Chinese break lines between any two characters, so a
line can end with one syllable (`す。`) alone below; where that shows on a page, a shorter
wording is the way out.

## Emoji names

The emoji picker's search and `:name` suggestions match the names and keywords of the
language in use, then English. They are not in the ARB files: `app/assets/emoji/<code>.tsv`
(`pt` is Brazilian, `zh` Simplified) holds the name and keywords of each emoji of
`emoji_table.dart`, made by `tools/emoji_names.py` from Unicode CLDR's annotations
(`cldr-annotations-full` and `cldr-annotations-derived-full` of a cldr-json release,
unpacked into a folder: `python3 -I tools/emoji_names.py <folder>`; data only, nothing in
it is run). The files carry CLDR 49.0.0 (Unicode License v3, `CLDR-LICENSE.txt`, listed in
`THIRD-PARTY-NOTICES`). The app reads the current language's file and English's the first
time the picker or a `:name` needs them, and folds case, accents (corazón = corazon,
cœur = coeur) and katakana (ハート = はーと) the same way for the data and the query. To
update, download the new release, rerun the script (it fails if CLDR lacks an emoji of the
table), update the version here and rerun `tools/third_party_notices.py` if the licence
text changed. A new language needs its CLDR locale added to `LANGUAGES` in the script and
a case in `emojiAssetOf` (`lib/src/emoji_names.dart`) if its code differs.

## Style

Plain, short and concrete, like the English, which is deliberately simple; never promising
more than the app does. The register of chat applications in each language: Spanish tú,
Brazilian Portuguese você, French tu, German du, Italian tu, Russian вы (lower case),
Japanese です/ます in sentences and short nouns on buttons, Chinese 你. Fogata and technical
names (UDP, NAT, IPv6, PNG, QR) stay. Buttons and tabs stay short; descriptions say where
room is tight.

Names come with no gender, so where a language marks it (Spanish, Italian, French,
Portuguese, Russian), prefer words that do not: a place rather than a person (“primer
puesto”, not “el primero”), “the other person” for one unnamed friend, a verb whose
participle does not agree. Where no short neutral wording reads naturally, the generic
masculine stays.

## Glossary

### Español (es, neutral, tú)

| English | Español |
|---|---|
| room / voice room | sala / sala de voz |
| group | grupo |
| friend | amigo (pl. amigos); for one unnamed person, “la otra persona” |
| friend request | solicitud de amistad |
| direct message / direct chat | mensaje directo / chat directo |
| call (noun) / to call | llamada / llamar |
| direct call | llamada directa |
| ring (a call rings) | sonar (la llamada suena); llamar a alguien |
| hang up | colgar |
| mute / unmute (microphone) | silenciar / activar el micrófono |
| mute for me (someone's voice) | silenciar para mí |
| deafen / undeafen | dejar de oír a los demás / volver a oír a los demás |
| microphone | micrófono |
| speaker (sound output) | altavoz |
| camera | cámara |
| video | video (not vídeo) |
| share the screen / share a window | compartir la pantalla / compartir una ventana |
| stream quality | calidad de transmisión |
| watch (a shared screen) | ver |
| invite / invitation / invite link | invitar / invitación / enlace de invitación |
| let in (a joiner) | dejar entrar |
| manager / manage the group | quienes gestionan el grupo / gestionar el grupo |
| member | miembro |
| admin | administrador |
| banner / panel background | banner / fondo del panel |
| profile / profile picture | perfil / foto de perfil |
| bio | biografía |
| move account (Settings part) / a move | Mover cuenta / mover la cuenta; el traslado |
| code | código |
| link | enlace |
| QR | código QR |
| notification / bell / notice | notificación / campana / aviso |
| toast | aviso (emergente) |
| settings | Ajustes |
| theme | tema |
| online | en línea |
| offline / appear offline | sin conexión / aparecer sin conexión (not “desconectado”, which has a gender) |
| typing | escribiendo |
| pinned / pin | fijado / fijar |
| disappearing messages | mensajes temporales |
| download / upload | descargar, descarga / subir, subida |
| file(s) | archivo(s) |
| picture | imagen (foto, for profile pictures) |
| time out (a person) / kept out | sacar (de una sala) por un tiempo; un tiempo fuera / sin acceso; until let back = hasta nuevo permiso |
| remove | quitar (from a list/group: sacar del grupo); delete = eliminar |
| leave | salir (de) |
| enter | entrar (en) |
| open / close (a room) | abrir / cerrar (la sala) |
| computer | equipo (neutral; not ordenador/computadora) |
| device | dispositivo |
| network / local network | red / red local |
| firewall | firewall |
| relay / through a member | a través de otro miembro |
| peer (network check) | nodo |
| router | router |
| connection | conexión |
| identity | identidad |
| fingerprint | huella |
| quit (the app) | salir de Fogata |
| recovery copy / recovery key | copia de recuperación / clave de recuperación |
| restore (a copy) | recuperar |
| keychain | llavero |
| microphone off (state) | micrófono silenciado |
| delete (a voice room) | eliminar (not cerrar) |
| the Fogata team | Fogata (never “equipo”, which here means computer) |
| first (in a group's order) | primer puesto |

#### Style rules
- Register: tú, informal and plain, as chat apps write in Spanish. Never vosotros or ustedes (say
  “los dos estén”, rephrase “ya sois amigos”). Neutral vocabulary that works
  in Spain and Latin America (equipo, not ordenador/computadora; enlace; archivo).
- Buttons, menu items and tooltips use the infinitive: Guardar, Cancelar, Salir de la sala.
- Sentence case everywhere (only the first word and names capitalised); headings that are
  in capitals in English stay in capitals.
- Punctuation: opening ¿ and ¡; quotes “…”; the ellipsis is one character …; keep “ · ”.
- Numbers and units: a space before units and before %: 5 min, 3 h, 30 s, 48,2 MB, 100 %
  (numbers come preformatted). Ordinals: {n}.º. Keep fps, ms, p (720p).
- Stays in English: Fogata, UDP, TCP, NAT, IPv4, IPv6, DNS, QR, PNG, JPEG, GIF, WebP,
  PowerShell, Terminal, Wi-Fi, router, firewall, FGT-XXXXX-XXXXX.
- macOS names: “Ajustes del Sistema › Privacidad y seguridad”.

### Português do Brasil (pt, app_pt.arb)

| English | Português (BR) |
|---|---|
| room / voice room | sala / sala de voz (fem.) |
| group | grupo |
| friend | amigo (generic masculine; "amigos") |
| friend request | pedido de amizade |
| direct message / chat | mensagem direta / conversa; tab and room chat: "Chat" (o chat) |
| call (noun) / call (verb) | chamada / ligar para |
| direct call | chamada direta |
| ring (it rings) | tocar (a chamada toca) |
| hang up | desligar |
| mute / unmute | silenciar / ativar (microfone): "Silenciar microfone", "Ativar microfone"; a person: "Silenciar para mim" |
| deafen / undeafen | parar de ouvir os outros / ouvir os outros de novo |
| microphone | microfone |
| speaker | alto-falante (device lists: dispositivo de saída) |
| camera | câmera |
| share screen / share a window | compartilhar a tela / compartilhar uma janela |
| stream quality | qualidade da transmissão |
| invite (verb) / invitation / invite link | convidar / convite / link de convite |
| manager / manage (the group) | quem administra o grupo / administrar o grupo |
| member | membro |
| admin (PowerShell) | administrador |
| banner / panel background | banner / fundo do painel |
| profile | perfil |
| bio | bio |
| move account / the move | transferir a conta / a transferência |
| code | código |
| link | link |
| QR | QR / código QR |
| notification / bell | notificação / sino (notices: avisos) |
| toast | aviso |
| settings | Configurações |
| theme | tema |
| online | online |
| offline / appear offline | offline / aparecer offline |
| typing | digitando |
| pinned / pin / unpin | fixada / fixar / desafixar |
| disappearing messages | mensagens temporárias |
| download / upload | baixar (download) / upload (bandwidth), enviar (files) |
| file(s) | arquivo(s) |
| picture | imagem; profile picture: foto do perfil |
| time out (a person) | suspender (de uma sala) por um tempo / suspensão; "took you out" = tirou você de |
| remove | remover; from a room: tirar de |
| leave | sair |
| enter | entrar |
| open (a room) / close (a room) | abrir / fechar (a sala fecha) |
| computer | computador |
| device | dispositivo |
| network / local network | rede / rede local |
| firewall | firewall |
| relay | retransmitir |
| peer | peer (network check only) |
| router | roteador |
| recovery copy / recovery key | cópia de recuperação / chave de recuperação |
| keychain | chaveiro |
| fingerprint | impressão digital |
| delete (room, chat, account) | excluir |
| log (technical) | registro |
| let back (after a time out) | liberar |
| files sent in chats | arquivos enviados nas conversas |

#### Style rules
- Register: "você", informal but neutral, as chat apps write in Brazilian Portuguese. Never "tu", never "o senhor".
- Buttons, menu items, tooltips and tabs: infinitive or short noun ("Cancelar", "Salvar", "Sair da sala", "Silenciar microfone"). Instructions inside sentences: imperative ("Peça um novo", "Tente de novo").
- Sentence case everywhere (only the first word and names in capitals). Headings the English writes in capitals stay in capitals.
- Quotation marks “…”; ellipsis "…" as one character; "·" separators and their spaces stay; "→" stays.
- Numbers come preformatted. Units with a space: "5 min", "3 h", "48 MB", "120 ms", "12 %" where English has the space; "100%" where English has none.
- Stay in English: Fogata, UDP, TCP, NAT, IPv4, IPv6, DNS, QR, PNG, JPEG, GIF, WebP, PowerShell, Terminal, Wi-Fi, peer, firewall, link, online/offline, banner, bio, chat, upload, fps, codes like FGT-XXXXX-XXXXX.
- macOS: "Ajustes do Sistema › Privacidade e Segurança" (macOS pt-BR names).

### Français (fr, tu)

| English | Français |
|---|---|
| room / voice room | salon / salon vocal |
| group | groupe |
| friend | ami |
| friend request | demande d’ami |
| direct message / direct chat | message privé / conversation privée |
| call / direct call | appel / appel direct |
| ring (a call) | sonner (« Appel en cours… » for “Calling…”) |
| mute / unmute | couper le micro / réactiver le micro; for a person: rendre muet / rétablir |
| deafen / undeafen | ne plus entendre les autres / réentendre les autres |
| microphone | micro (microphone in long labels) |
| speaker (sound output) | haut-parleur / sortie audio |
| camera | caméra |
| share screen / share a window | partager l’écran / partager une fenêtre |
| stream quality | qualité du partage |
| invite / invitation / invite link | inviter / invitation / lien d’invitation |
| manager / manage (the group) | ceux qui gèrent le groupe / gérer (le groupe) |
| member | membre |
| admin | admin |
| banner / panel background | bannière / fond du panneau |
| profile | profil |
| bio | bio |
| move account | transférer le compte |
| code | code |
| link | lien |
| QR | code QR |
| notification / bell | notification / cloche (panneau des notifications) |
| toast | message (not visible to users) |
| settings | paramètres |
| theme | thème |
| online | en ligne |
| offline / appear offline | hors ligne / apparaître hors ligne |
| typing | écrit… (« Marta écrit… ») |
| pinned / pin / unpin | épinglé / épingler / désépingler |
| disappearing messages | messages éphémères |
| download | télécharger / téléchargement |
| upload (bandwidth) | débit montant / envoi |
| file(s) | fichier(s) |
| picture / profile picture | image / photo de profil |
| time out (a person) | exclure temporairement / exclusion temporaire |
| remove | retirer |
| leave | quitter |
| enter (a room) | entrer dans / rejoindre |
| open (a room) / close (a room) | ouvrir / fermer (le salon se ferme) |
| computer | ordinateur |
| device | appareil |
| network | réseau |
| firewall | pare-feu |
| relay | relais / relayer |
| fingerprint | empreinte |
| peer | pair |
| router | routeur |
| connection | connexion |
| file sizes B / KB / MB / GB / TB | o / Ko / Mo / Go / To |
| direct messages (list, storage) | messages privés |
| recovery copy / recovery key | copie de secours / clé de secours |
| keychain | trousseau |
| let in / let back | faire entrer / réintégrer |
| kept out (until let back) | exclu (jusqu’à nouvel ordre) |
| delete (a voice room) | supprimer |
| chat (of a room or call) | chat |
| first (in a group's order) | premier rang |

#### Style rules
- Register: tutoiement everywhere (« tu », « ton », « tes »), plain and short, as chat apps write in French.
- Typography: no-break space (U+00A0) before « : ; ? ! » and before « % », and inside guillemets « … »; typographic apostrophe ’; “…” for one ellipsis character.
- Capitalisation: sentence case only (first word and proper nouns); headings in capitals stay in capitals.
- Buttons and menu items: infinitive (« Envoyer », « Réessayer », « Quitter le salon »); tooltips also infinitive. Status labels: short nouns or participles.
- Numbers and units: decimal formatting comes in already formatted; units after a no-break space (« 5 min », « 3 h », « 48,2 Mo », « 30 i/s » → kept as « 30 fps » for frame rates as French apps do).
- Stays in English / as is: Fogata, UDP, TCP, NAT, IPv4, IPv6, DNS, PNG, JPEG, GIF, WebP, PowerShell, Terminal, Wi-Fi, FGT-XXXXX-XXXXX, Ctrl, Maj (Shift is written « Maj » on French keyboards).
- Plurals: `one` (covers 0 and 1) and `other`; inside `one` use {n} where English has a number.

### Deutsch (de, du)

| English | Deutsch |
|---|---|
| room / voice room | Raum / Sprachraum |
| group | Gruppe |
| friend | Freund (pl. Freunde; generic, also for women) |
| friend request | Freundschaftsanfrage |
| direct message / direct chat | Direktnachricht / Direktchat |
| call (noun) / call (verb) | Anruf (also ongoing: „im Anruf“) / anrufen |
| direct call | Direktanruf |
| ring / ringing | klingeln / Klingelt |
| hang up | Auflegen |
| mute / unmute | Stummschalten / Stummschaltung aufheben (mic button: Mikrofon stummschalten / Mikrofon einschalten) |
| deafen / undeafen | Die anderen nicht mehr hören / Die anderen wieder hören |
| microphone | Mikrofon |
| speaker (output) | Lautsprecher |
| camera | Kamera |
| share screen / share a window | Bildschirm teilen / Fenster teilen |
| stream quality | Streamqualität |
| invite / invitation / invite link | einladen / Einladung / Einladungslink |
| manager / manage (the group) | Verwalter / die Gruppe verwalten („die, die die Gruppe verwalten“) |
| member | Mitglied |
| admin | Admin |
| banner / panel background | Banner / Hintergrund |
| profile | Profil |
| bio | Bio |
| move account / the move | Konto umziehen / Umzug |
| code | Code |
| link | Link |
| QR | QR-Code |
| notification / bell | Benachrichtigung / Glocke |
| toast | Hinweis |
| settings | Einstellungen (Voice & video section: Audio & Video, to avoid clashing with Sprache = language) |
| theme | Design |
| online / offline / appear offline | Online / Offline / als offline angezeigt werden |
| typing | schreibt … |
| pinned / pin / unpin | Angepinnt / Anpinnen / Lösen |
| disappearing messages | Verschwindende Nachrichten |
| download / upload | Herunterladen / Upload (bandwidth: Upload) |
| file(s) | Datei(en) |
| picture | Bild (profile: Profilbild) |
| time out (a person) / kept out / let back | aussperren / Ausgesperrt / Freigeben; „bis auf Weiteres“ = until let back |
| remove | Entfernen |
| leave | Verlassen |
| enter (a room) | Betreten |
| open (a room) | Öffnen |
| close (a room) | Schließen |
| computer | Computer |
| device | Gerät |
| network | Netzwerk |
| firewall | Firewall |
| relay (member in between) | Weiterleitung über ein Mitglied |
| peer | Peer |
| router | Router |
| identity | Identität |
| keychain | Schlüsselbund |
| recovery copy / recovery key | Wiederherstellungskopie / Wiederherstellungsschlüssel |
| disk | Datenträger |
| tray | Infobereich |
| fingerprint | Fingerabdruck |
| away | Abwesend |
| undone (activity) | Rückgängig gemacht |

#### Style rules
- Register: informal „du“ (lower case du/dich/dein), as chat apps write in German. Plain and short, no added warmth.
- Buttons and menu items: infinitive („Speichern“, „Raum verlassen“), not imperative. Tooltips likewise. Sentences in messages may use imperative („Versuch es erneut.“).
- Capitalisation: German noun rules; titles and buttons in sentence case (first word capital, nouns capital).
- Quotes: „…“; apostrophe ’ only where German needs one (rarely; „Martas Bildschirm“, with names ending in s/x/z „Max’ Bildschirm“ — we use „Bildschirm von {name}“ to avoid it).
- Ellipsis: one character with a space before it, as Duden writes it: „Löschen …“, „Wird geladen …“.
- Numbers and units: space before units and %: „5 min“, „3 Std.“, „48,2 MB“, „12 %“. Numbers come pre-formatted.
- Keep in English: Fogata, UDP, TCP, NAT, IPv4, IPv6, DNS, QR, PNG, JPEG, GIF, WebP, PowerShell, Terminal, Wi-Fi → „WLAN“, fps stays „fps“.
- macOS paths: „Systemeinstellungen › Datenschutz & Sicherheit“.
- Ordinals: „{n}.“ ; list: „A, B und C“ (no Oxford comma).

### Italiano (it, tu)

| English | Italiano |
|---|---|
| room / voice room | stanza / stanza vocale |
| open a room / close a room | aprire / chiudere la stanza (the room "closes" = si chiude) |
| enter (a room) | entrare (in) |
| leave | uscire (da); button: Esci |
| group | gruppo |
| friend | amico (pl. amici) |
| friend request | richiesta di amicizia |
| direct message / direct chat | messaggio diretto / chat diretta |
| call / direct call | chiamata / chiamata diretta |
| ring | squillare (far squillare) |
| hang up | riagganciare; button: Riaggancia |
| mute / unmute (own mic) | disattiva il microfono / attiva il microfono; state: microfono disattivato |
| mute (someone, for me) | silenzia (per me) |
| deafen / undeafen | disattiva l’audio / riattiva l’audio |
| microphone | microfono |
| speaker (output) | altoparlante |
| camera | videocamera |
| share screen / share a window | condividi lo schermo / condividi una finestra |
| stream quality | qualità della trasmissione |
| invite (verb) / invitation / invite link | invitare / invito / link d’invito |
| manager / manage the group | chi gestisce il gruppo / gestire il gruppo |
| member | membro |
| admin | amministratore |
| banner / panel background | banner / sfondo del pannello |
| profile / bio | profilo / bio |
| profile picture / picture | immagine del profilo / immagine |
| move account | trasferire l’account |
| code / link / QR | codice / link / codice QR |
| notification / bell | notifica / campanella |
| toast | avviso |
| settings | Impostazioni |
| theme | tema |
| online / offline / appear offline | online / offline / apparire offline |
| typing | sta scrivendo |
| pinned | fissato (messaggi fissati) |
| disappearing messages | messaggi effimeri |
| download / upload | scaricare / caricare; bandwidth: upload |
| file(s) | file (invariable) |
| time out (a person) / a time out | sospendere / sospensione |
| remove | rimuovere (from a room: far uscire / togliere) |
| computer / device | computer / dispositivo |
| network / local network | rete / rete locale |
| firewall / router / relay | firewall / router / relay |
| peer | peer |
| connection | connessione |
| fingerprint (short code) | impronta |
| recovery copy / recovery key | copia di recupero / chiave di recupero |
| keychain | portachiavi |
| kept out / let back | escluso / far rientrare |
| type (a code) | digitare |
| away (status) | assente |
| card (profile) | scheda |
| first (in a group's order) | primo posto |
| seen (last online) | ultimo accesso |
| entered / joined (notes) | ha raggiunto (no participle agreement) |

#### Style rules
- Register: informal **tu**, as chat apps write in Italian. Plain and short; no added warmth.
- Buttons and menu items: imperative 2nd person (Salva, Annulla, Invia, Esci, Copia il codice); tooltips the same. Labels and titles as nouns.
- Sentence case everywhere (only the first word and proper names capitalised). Small headings in capitals stay in capitals.
- Quotes: “…” (as the English source). Apostrophe: ’ (typographic). Ellipsis: one character "…". Keep "·" separators and "›" menu paths.
- Numbers come formatted; units: min, h, s, ms, KB, MB, GB, fps, with a space before the unit; percent with no space, as Italian writes it ({n}%).
- Stay as they are: Fogata, UDP, TCP, NAT, IPv4, IPv6, DNS, PowerShell, PNG, JPEG, GIF, WebP, Wi-Fi, codes FGT-XXXXX-XXXXX. The macOS Terminal app is "Terminale" in Italian macOS.
- Ordinals: {n}º. Times: "ieri, {time}".
- macOS paths in Italian: "Impostazioni di Sistema › Privacy e sicurezza".

### Русский (ru, вы)

| English | Русский |
|---|---|
| room / voice room | комната / голосовая комната |
| group | группа |
| friend | друг (мн. друзья) |
| friend request | запрос в друзья |
| direct message / direct chat | личное сообщение / личный чат |
| chat | чат |
| call / direct call | звонок / личный звонок |
| ring (a call rings) | звонить; «Звонок…», «Вызов» |
| mute / unmute (own microphone) | выключить микрофон / включить микрофон |
| mute for me (someone else) | заглушить для меня |
| deafen / undeafen | выключить звук / снова слышать других |
| microphone | микрофон |
| speaker (sound output) | динамик; устройство вывода |
| camera | камера |
| share screen / share a window | показать экран / показать окно; показ экрана |
| stream quality | качество трансляции |
| invite / invitation / invite link | пригласить / приглашение / ссылка-приглашение |
| manager / manage the group | управляющий / управлять группой («те, кто управляет группой») |
| member | участник |
| admin (system) | администратор |
| banner / panel background | баннер / фон панели |
| profile | профиль |
| profile picture | аватар |
| bio | описание («О себе» as a field title) |
| identity (account) | аккаунт (never «личность») |
| move account | перенос аккаунта / перенести аккаунт |
| code | код |
| link | ссылка |
| QR | QR-код |
| notification / bell | уведомление / колокольчик |
| toast | всплывающее сообщение |
| settings | настройки |
| theme | тема |
| online | в сети |
| offline / appear offline | не в сети / невидимка (казаться не в сети) |
| typing | печатает |
| pinned | закреплённые |
| disappearing messages | исчезающие сообщения |
| download / upload | скачать, скачивание / отправка; исходящий канал (bandwidth) |
| file(s) | файл(ы) |
| picture | изображение (картинка in casual places) |
| time out (a person) | временно отстранить; тайм-аут |
| remove | удалить / убрать |
| leave | выйти, покинуть |
| enter | войти |
| open / close (a room) | открыть / закрыть (комната закрывается) |
| computer | компьютер |
| device | устройство |
| network | сеть |
| firewall | брандмауэр |
| relay / through a member | через участника (who passes traffic on) |
| peer | узел |
| fingerprint | отпечаток (код-отпечаток where “code” matters) |
| recovery copy / recovery key | резервная копия / ключ восстановления |
| keychain | связка ключей |
| try again | «Повторить» (button) / «Повторите попытку» (sentence) |
| copy (button) | Копировать |

#### Style rules
- Register: «вы» in lower case, plain and neutral, as chat apps write in Russian. No exclamation marks unless English has them.
- Buttons, menu items and tooltips use the infinitive: «Отправить», «Выйти из комнаты», «Выключить микрофон». Titles and labels are nouns where natural.
- Sentence case everywhere (only the first word capitalized). Quotes «…», nested „…“. Ellipsis is one character «…». Use «ё» consistently (всё, ещё, закреплённые).
- Names ({name}, {who}, {group}, {room}) are treated as indeclinable: keep them in the nominative — after a generic noun («участник {name}», «в группе «{group}»», «в комнате {room}») or as «{name}: …». Avoid past-tense verbs whose ending would reveal gender after a bare name; use present tense, a generic subject («Пользователь», «Участник») or impersonal forms.
- Plurals always have one/few/many/other with {n} in every case (one = 1, 21, 31…; other = fractions, genitive singular).
- Units: Б, КБ, МБ, ГБ, ТБ, мс, с, мин, ч, кадр/с; space between number and unit; percent written «{n}%».
- Stay in Latin script: Fogata, UDP, TCP, NAT, IPv4, IPv6, DNS, Wi-Fi, PNG, JPEG, GIF, WebP, PowerShell, FGT-XXXXX-XXXXX. macOS names as macOS shows them in Russian: «Системные настройки › Конфиденциальность и безопасность», «Терминал».

### 日本語 (ja)

| English | 日本語 |
|---|---|
| room / voice room | ルーム / ボイスルーム |
| group | グループ |
| friend | フレンド |
| friend request | フレンド申請 |
| direct message / direct chat | ダイレクトメッセージ（DM） / ダイレクトチャット |
| chat | チャット |
| call / direct call | 通話 / ダイレクト通話 |
| ring (a call) | 呼び出す / 呼び出し中 |
| mute / unmute | ミュート / ミュート解除 |
| deafen / undeafen | スピーカーミュート / スピーカーミュート解除 |
| microphone | マイク |
| speaker | スピーカー |
| camera | カメラ |
| share screen / share a window | 画面を共有 / ウィンドウを共有（名詞：画面共有） |
| stream quality | 配信画質 |
| watch (a shared screen) | 視聴 |
| invite / invitation / invite link | 招待 / 招待 / 招待リンク |
| manager / manage the group | 管理者 / グループを管理する |
| member | メンバー |
| admin (PowerShell) | 管理者 |
| banner / panel background | バナー / パネルの背景 |
| profile | プロフィール |
| profile picture | プロフィール画像 |
| bio | 自己紹介 |
| move account | アカウントの移行（動詞：アカウントを移行） |
| code | コード |
| link | リンク |
| QR | QRコード |
| notification / bell | 通知 / ベル |
| toast | トースト（画面には出さない） |
| settings | 設定 |
| theme | テーマ |
| online | オンライン |
| offline / appear offline | オフライン / オフライン表示 |
| typing | 入力中 |
| pinned / pin / unpin | ピン留め / ピン留め / ピン留めを解除 |
| disappearing messages | 消えるメッセージ |
| download / upload | ダウンロード / アップロード（回線：上り） |
| file(s) | ファイル |
| picture | 画像 |
| time out (a person) | タイムアウト |
| remove (from group) | グループから外す |
| remove (from room) | ルームから退出させる |
| remove (a file, an item) | 削除 / 取り除く |
| leave | 退出 |
| enter (a room) | 入室 / ルームに入る |
| open (a room) | ルームを開く |
| close (a room) | ルームを閉じる |
| computer | パソコン |
| device | デバイス |
| network | ネットワーク |
| firewall | ファイアウォール |
| relay / through a member | 中継 / メンバー経由 |
| peer / router | ピア / ルーター |
| identity | ID |
| fingerprint | フィンガープリント |
| account | アカウント |
| connection | 接続 |
| live | ライブ |
| recovery copy / recovery key | 復元用コピー / 復元キー |
| keychain | キーチェーン |
| request to join / let in | 参加申請 / 承認 |
| the other computer (account move) | もう一方のパソコン |
| default | デフォルト |
| block (firewall, network) | ブロック |
| folder | フォルダー |
| above / below you (group order) | 上の人 / 下の人; the first n: 上位{n}人 |
| names | without さん |

#### Style rules
- Sentences in polite です/ます; buttons, tabs, tooltips and labels as short nouns or verb
  stems (送信, キャンセル, 退出, ミュート解除). Toasts and notes are full sentences ending in 。
- No spaces between Japanese words; counters stick to numbers (5分前, 3人, 2日). Latin unit
  abbreviations keep a space as in Windows (48.2 MB, 30 fps, 120 ms); percent has none (12%).
- Full-width punctuation: 。、！？：（）; quotes 「…」 (nested 『…』). "…" stays one
  character; "·" separators stay as in English with their spaces.
- Where English capitalises a heading in capitals (LIVE, BELOW YOU), use plain Japanese.
- Sentence parts that follow other words stay parts; no capitalisation concerns.
- Stays in Latin script: Fogata, UDP, TCP, NAT, IPv4, IPv6, DNS, QR, PowerShell, Wi-Fi,
  PNG, JPEG, GIF, WebP, FGT-XXXXX-XXXXX; macOS app/menu names as macOS Japanese prints them
  (ターミナル, システム設定 › プライバシーとセキュリティ, 一般 › ログイン項目).
- No warmth, emoji or promises beyond the English; plain and short.

### 简体中文 (zh, zh-Hans, mainland)

| English | 中文 |
|---|---|
| room / voice room | 房间 / 语音房间 |
| open (a room) / close (a room) | 开启 / 关闭 |
| live (room) | 进行中 |
| group | 群组 |
| friend | 好友 |
| friend request | 好友请求 |
| friend link / friend code | 好友链接 / 好友码 |
| direct message / direct chat | 私信 / 私聊 |
| call / direct call | 通话 / 私人通话 |
| ring (a call rings) | 呼叫 / 响铃 |
| answer / decline / hang up | 接听 / 拒绝 / 挂断 |
| mute / unmute | 静音 / 取消静音 |
| deafen / undeafen | 不再听到其他人 / 重新听到其他人 |
| microphone | 麦克风 |
| speaker (output) | 扬声器 |
| camera | 摄像头 |
| share screen / share a window | 共享屏幕 / 共享窗口 |
| stream quality | 共享画质 |
| invite / invitation / invite link | 邀请 / 邀请 / 邀请链接 |
| manager / manage (the group) | 管理者 / 管理（群组） |
| those who manage the group | 群组管理者 |
| member | 成员 |
| admin | 管理员 |
| banner / panel background | 横幅 / 面板背景 |
| profile / profile picture | 个人资料 / 头像 |
| bio | 个人简介 |
| identity | 身份 |
| move account | 迁移账号 |
| code | 码（迁移码、好友码） |
| link | 链接 |
| QR | 二维码 |
| notification / bell | 通知 / 通知铃铛 |
| toast | 提示 |
| settings | 设置 |
| theme | 主题 |
| online / offline / appear offline | 在线 / 离线 / 显示为离线 |
| away | 暂离 |
| typing | 正在输入 |
| pinned / pin | 已置顶 / 置顶 |
| disappearing messages | 限时消息 |
| download / upload | 下载 / 上传 |
| file(s) | 文件 |
| picture | 图片 |
| time out (a person) | 暂时移出 |
| remove | 移除 |
| leave (room) / leave (group) | 离开 / 退出 |
| enter / join | 进入 / 加入 |
| let in | 同意（某人）加入；button: 同意 |
| computer | 电脑 |
| device | 设备 |
| network / local network | 网络 / 局域网 |
| firewall | 防火墙 |
| relay / carrier member | 中继 / 中转成员 |
| connection / delay / lost packets | 连接 / 延迟 / 丢包 |
| keychain | 钥匙串 |
| fingerprint | 指纹 |
| let back (into a room) | 放行 / 让……回来 |
| recovery copy / recovery key | 恢复副本 / 恢复密钥 |
| above / below (group order) | 排在……上面 / 下面 |
| first (member) | 排第一的人 / 第一位 |
| mute / unmute microphone | 麦克风静音 / 取消麦克风静音 |
| status | 状态 |

#### Style rules
- Register: 你 (never 您), plain and concise, as chat apps write in mainland China. No warmth added, no emoji.
- Punctuation: full-width ，。！？：；（） and quotes “…”; “…” around group/room names where English has them. "…" stays one character (…). " · " separators stay as in English (half-width with spaces).
- Spacing: no space between Chinese characters and Latin words, numbers or placeholders (5分钟前, {n}个文件, 在Fogata中). A Latin unit after a number keeps its Latin space: {n} MB, {n} ms, {n} fps, 720p. Percent written {n}% with no space. Exceptions, as Chinese apps write them: a clock time after a day keeps one space (昨天 23:23, {date} {time}), and a label–value legend keeps one (图片 48.2 MB). Avoid 开房 (innuendo): say 房间上次开启于….
- Buttons and menu items: short verb phrases (保存, 挂断, 离开并关闭); titles as noun phrases; tooltips as short verb phrases.
- Plurals: only `other` (Chinese does not inflect); =0/=1 only where wording differs. Measure words: 个文件, 条消息, 位成员 → use 个/条/人 plainly.
- Stays in English: Fogata, UDP, TCP, NAT, IPv4, IPv6, DNS, PNG, JPEG, GIF, WebP, PowerShell, Wi-Fi, FGT-XXXXX-XXXXX. macOS names: 系统设置 › 隐私与安全性, 终端.
- Lower-case sentence parts in English: just the Chinese phrase (no capitalisation exists).
