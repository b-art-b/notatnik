// A compact emoji set for `:shortcode` completion and the emoji picker. The notes store the emoji itself (a plain
// Unicode character), never a shortcode, so files stay readable everywhere. `name` is the shortcode; the rest are
// search words.

export type EmojiGroup =
  'Smileys' | 'People' | 'Animals & nature' | 'Food' | 'Activities' | 'Travel' | 'Objects' | 'Symbols'
export interface Emoji {
  char: string
  name: string
  words: string
  group: EmojiGroup
}

// group | emoji name search words
const RAW = `
Smileys|😀 grinning happy smile
Smileys|😃 smiley happy joy
Smileys|😄 smile happy laugh
Smileys|😁 grin beam teeth
Smileys|😆 laughing satisfied
Smileys|😅 sweat_smile relief nervous
Smileys|😂 joy tears laugh lol
Smileys|🤣 rofl rolling laugh
Smileys|🙂 slight_smile fine
Smileys|🙃 upside_down silly
Smileys|😉 wink
Smileys|😊 blush happy shy
Smileys|😇 innocent angel halo
Smileys|🥰 smiling_hearts love adore
Smileys|😍 heart_eyes love crush
Smileys|🤩 star_struck wow excited
Smileys|😘 kissing_heart kiss
Smileys|😋 yum tasty delicious
Smileys|😛 tongue playful
Smileys|😜 wink_tongue crazy joke
Smileys|🤪 zany goofy
Smileys|🤗 hugs hug
Smileys|🤔 thinking hmm consider
Smileys|🤨 raised_eyebrow doubt
Smileys|😐 neutral meh
Smileys|😑 expressionless
Smileys|😶 no_mouth silent
Smileys|🙄 eyeroll annoyed
Smileys|😏 smirk
Smileys|😒 unamused
Smileys|😞 disappointed sad
Smileys|😔 pensive sad
Smileys|😟 worried
Smileys|😕 confused
Smileys|🙁 frown sad
Smileys|😣 persevere struggle
Smileys|😖 confounded
Smileys|😫 tired weary
Smileys|😩 weary tired
Smileys|🥺 pleading puppy
Smileys|😢 cry sad tear
Smileys|😭 sob crying loud
Smileys|😤 triumph huff proud
Smileys|😠 angry mad
Smileys|😡 rage angry red
Smileys|🤬 cursing swearing
Smileys|😱 scream fear shocked
Smileys|😨 fearful scared
Smileys|😰 anxious cold_sweat
Smileys|😥 sad_relieved
Smileys|😓 downcast_sweat
Smileys|🤯 mind_blown exploding_head
Smileys|😳 flushed embarrassed
Smileys|🥵 hot overheated
Smileys|🥶 cold freezing
Smileys|😴 sleeping zzz sleep tired
Smileys|😪 sleepy
Smileys|🤤 drooling
Smileys|😷 mask sick ill
Smileys|🤒 thermometer sick fever
Smileys|🤕 bandage hurt injured
Smileys|🤢 nauseated sick
Smileys|🤮 vomiting sick
Smileys|🥳 partying party celebrate
Smileys|😎 sunglasses cool
Smileys|🤓 nerd geek glasses
Smileys|🧐 monocle curious
Smileys|🤠 cowboy
Smileys|🥸 disguise
Smileys|🤡 clown
Smileys|👻 ghost halloween
Smileys|💀 skull dead
Smileys|👽 alien
Smileys|🤖 robot bot
Smileys|💩 poop
Smileys|😈 devil smiling_imp
People|👍 thumbsup +1 yes ok approve like
People|👎 thumbsdown -1 no dislike
People|👏 clap applause bravo
People|🙌 raised_hands hooray celebrate
People|🙏 pray thanks please folded_hands
People|🤝 handshake deal agreement
People|✌️ victory peace
People|🤞 crossed_fingers luck hope
People|👌 ok_hand perfect
People|🤌 pinched_fingers
People|👋 wave hello bye hi
People|✋ raised_hand stop
People|👉 point_right
People|👈 point_left
People|👆 point_up
People|👇 point_down
People|☝️ point_up_2 one
People|💪 muscle strong flex
People|🧠 brain mind smart
People|👀 eyes look see watching
People|👁️ eye
People|👂 ear listen
People|👃 nose
People|👄 mouth lips
People|👤 bust_in_silhouette person user
People|👥 busts_in_silhouette people team group
People|🧑 person adult
People|👨 man
People|👩 woman
People|👶 baby
People|🧑‍💻 technologist developer coder computer
People|🧑‍💼 office_worker business manager
People|🧑‍🔧 mechanic engineer technician
People|🧑‍🏫 teacher
People|🧑‍⚕️ health_worker doctor
People|🧑‍🎨 artist
People|🧑‍🚀 astronaut
People|🧙 mage wizard
People|🦸 superhero hero
People|🤷 shrug dunno
People|🤦 facepalm
People|🙋 raising_hand question volunteer
People|💁 info_desk
People|🏃 running runner
People|🚶 walking
People|🧘 yoga meditate calm
Animals & nature|🐶 dog puppy pet
Animals & nature|🐱 cat kitten pet
Animals & nature|🐭 mouse
Animals & nature|🐰 rabbit bunny
Animals & nature|🦊 fox
Animals & nature|🐻 bear
Animals & nature|🐼 panda
Animals & nature|🐨 koala
Animals & nature|🐯 tiger
Animals & nature|🦁 lion
Animals & nature|🐮 cow
Animals & nature|🐷 pig
Animals & nature|🐸 frog
Animals & nature|🐵 monkey
Animals & nature|🐔 chicken
Animals & nature|🐧 penguin
Animals & nature|🐦 bird
Animals & nature|🦉 owl wise
Animals & nature|🦅 eagle
Animals & nature|🐝 bee honey busy
Animals & nature|🐛 bug caterpillar
Animals & nature|🦋 butterfly
Animals & nature|🐌 snail slow
Animals & nature|🐞 ladybug
Animals & nature|🐢 turtle slow
Animals & nature|🐍 snake python
Animals & nature|🐙 octopus
Animals & nature|🐬 dolphin
Animals & nature|🐳 whale
Animals & nature|🦈 shark
Animals & nature|🐘 elephant
Animals & nature|🦄 unicorn
Animals & nature|🌱 seedling sprout plant grow
Animals & nature|🌿 herb plant leaf
Animals & nature|🌳 tree deciduous
Animals & nature|🌲 evergreen pine tree
Animals & nature|🍀 clover luck four_leaf
Animals & nature|🌸 cherry_blossom flower spring
Animals & nature|🌹 rose flower love
Animals & nature|🌻 sunflower flower
Animals & nature|🌷 tulip flower
Animals & nature|🌍 earth_europe world globe
Animals & nature|🌙 moon crescent night
Animals & nature|⭐ star favorite
Animals & nature|🌟 glowing_star shine
Animals & nature|✨ sparkles magic new clean
Animals & nature|⚡ zap lightning fast power
Animals & nature|🔥 fire hot lit flame
Animals & nature|💧 droplet water
Animals & nature|🌊 wave ocean sea
Animals & nature|☀️ sun sunny
Animals & nature|⛅ partly_sunny cloud
Animals & nature|☁️ cloud
Animals & nature|🌧️ rain cloud_rain
Animals & nature|⛈️ storm thunder
Animals & nature|❄️ snowflake snow cold winter
Animals & nature|🌈 rainbow
Food|🍎 apple red fruit
Food|🍏 green_apple fruit
Food|🍊 orange tangerine fruit
Food|🍋 lemon fruit
Food|🍌 banana fruit
Food|🍉 watermelon fruit
Food|🍇 grapes fruit
Food|🍓 strawberry fruit
Food|🍒 cherries fruit
Food|🍑 peach fruit
Food|🥑 avocado
Food|🍅 tomato
Food|🥕 carrot
Food|🌽 corn
Food|🥦 broccoli
Food|🍞 bread
Food|🥐 croissant
Food|🧀 cheese
Food|🍳 egg cooking breakfast
Food|🥓 bacon
Food|🍔 hamburger burger
Food|🍟 fries
Food|🍕 pizza
Food|🌮 taco
Food|🍝 spaghetti pasta
Food|🍜 ramen noodles soup
Food|🍣 sushi
Food|🍰 cake dessert
Food|🎂 birthday cake
Food|🍪 cookie
Food|🍫 chocolate
Food|🍩 donut
Food|🍦 ice_cream
Food|☕ coffee tea hot break
Food|🍵 tea matcha
Food|🍺 beer drink
Food|🍷 wine glass drink
Food|🥂 clinking_glasses cheers toast
Food|🍾 champagne celebrate
Food|🥤 cup drink
Activities|⚽ soccer football
Activities|🏀 basketball
Activities|🏈 american_football
Activities|⚾ baseball
Activities|🎾 tennis
Activities|🏐 volleyball
Activities|🏓 ping_pong
Activities|🥊 boxing
Activities|⛳ golf
Activities|🎯 dart target goal bullseye
Activities|🎮 video_game gaming
Activities|🎲 game_die dice random
Activities|🧩 puzzle piece jigsaw
Activities|♟️ chess strategy
Activities|🎨 art palette paint design
Activities|🎬 clapper movie film
Activities|🎤 microphone sing
Activities|🎧 headphones music
Activities|🎵 music note
Activities|🎸 guitar
Activities|🎹 piano keyboard
Activities|🏆 trophy win award first
Activities|🥇 gold medal first
Activities|🥈 silver medal second
Activities|🥉 bronze medal third
Activities|🏅 sports_medal
Activities|🎉 tada party celebrate congrats
Activities|🎊 confetti
Activities|🎁 gift present
Activities|🎈 balloon party
Travel|🚗 car automobile
Travel|🚕 taxi
Travel|🚌 bus
Travel|🚆 train
Travel|✈️ airplane plane flight travel
Travel|🚀 rocket launch ship fast
Travel|🚲 bike bicycle
Travel|🏍️ motorcycle
Travel|🚢 ship boat
Travel|⛵ sailboat
Travel|🏠 house home
Travel|🏡 house_garden home
Travel|🏢 office building company work
Travel|🏭 factory industry
Travel|🏥 hospital health
Travel|🏫 school
Travel|🏦 bank money
Travel|🏨 hotel
Travel|⛪ church
Travel|🗼 tower
Travel|🗽 statue_liberty
Travel|🏖️ beach vacation holiday
Travel|🏔️ mountain
Travel|🗺️ map world
Travel|🧭 compass direction
Travel|⏰ alarm_clock time
Travel|⌛ hourglass time wait
Travel|⏳ hourglass_flowing wait scheduled
Travel|🕐 clock time one
Objects|💻 laptop computer work
Objects|🖥️ desktop computer
Objects|⌨️ keyboard
Objects|🖱️ mouse computer
Objects|📱 phone mobile smartphone
Objects|☎️ telephone call
Objects|📞 receiver call
Objects|🔋 battery
Objects|🔌 plug electric
Objects|💡 bulb idea light
Objects|🔦 flashlight
Objects|📷 camera photo
Objects|📹 video camera
Objects|📺 tv television
Objects|📻 radio
Objects|🎙️ studio_microphone podcast
Objects|📡 satellite antenna
Objects|💾 floppy_disk save
Objects|💿 cd disc
Objects|📀 dvd
Objects|📁 folder file_folder
Objects|📂 open_folder
Objects|📄 page document file
Objects|📃 page_curl
Objects|📑 bookmark_tabs
Objects|📊 bar_chart stats report
Objects|📈 chart_up growth increase
Objects|📉 chart_down decrease
Objects|📋 clipboard list checklist
Objects|📌 pushpin pin important
Objects|📍 round_pushpin location place
Objects|📎 paperclip attachment
Objects|🖇️ linked_paperclips
Objects|📏 ruler
Objects|✂️ scissors cut
Objects|🗃️ card_file_box archive
Objects|🗄️ file_cabinet archive
Objects|🗑️ wastebasket trash delete
Objects|🔒 lock secure private
Objects|🔓 unlock open
Objects|🔑 key password
Objects|🔨 hammer build tool
Objects|🔧 wrench tool fix settings
Objects|🔩 nut_and_bolt
Objects|⚙️ gear settings config
Objects|🧰 toolbox
Objects|🧲 magnet
Objects|🔬 microscope science
Objects|🔭 telescope
Objects|💊 pill medicine
Objects|💉 syringe
Objects|🧪 test_tube experiment
Objects|📚 books library read
Objects|📖 open_book read
Objects|📕 closed_book red
Objects|📗 green_book
Objects|📘 blue_book
Objects|📙 orange_book
Objects|📓 notebook
Objects|📒 ledger
Objects|📝 memo note write pencil todo
Objects|✏️ pencil write
Objects|🖊️ pen write
Objects|🖋️ fountain_pen
Objects|📧 email mail
Objects|✉️ envelope mail letter
Objects|📨 incoming_envelope
Objects|📤 outbox
Objects|📥 inbox_tray
Objects|📦 package box delivery
Objects|📬 mailbox
Objects|📅 calendar date schedule deadline
Objects|📆 tear_off_calendar date
Objects|🗓️ spiral_calendar date plan
Objects|🔔 bell notification alert
Objects|🔕 no_bell mute
Objects|📢 loudspeaker announce
Objects|📣 megaphone announce
Objects|🔍 mag search find zoom
Objects|🔎 mag_right search
Objects|🏷️ label tag price
Objects|💰 moneybag money rich
Objects|💵 dollar money cash
Objects|💶 euro money cash
Objects|💳 credit_card payment
Objects|💎 gem diamond
Objects|⚖️ balance_scale justice law
Objects|🛒 shopping_cart buy
Objects|🧾 receipt invoice
Objects|🪪 id_card
Objects|👔 necktie suit business
Objects|🎓 graduation_cap education
Objects|👑 crown king queen
Symbols|❤️ heart love red
Symbols|🧡 orange_heart
Symbols|💛 yellow_heart
Symbols|💚 green_heart
Symbols|💙 blue_heart
Symbols|💜 purple_heart
Symbols|🖤 black_heart
Symbols|🤍 white_heart
Symbols|💔 broken_heart
Symbols|💕 two_hearts
Symbols|💯 hundred perfect score
Symbols|✅ white_check_mark done check yes complete
Symbols|✔️ heavy_check_mark done check
Symbols|☑️ ballot_box_with_check done
Symbols|❌ x cross no wrong delete
Symbols|❎ negative_squared_cross_mark
Symbols|⭕ o circle
Symbols|❗ exclamation important warning
Symbols|❓ question what help
Symbols|‼️ bangbang important
Symbols|⚠️ warning caution alert
Symbols|🚫 no_entry_sign forbidden stop
Symbols|⛔ no_entry
Symbols|🛑 stop_sign
Symbols|ℹ️ information info
Symbols|♻️ recycle
Symbols|➕ heavy_plus plus add
Symbols|➖ heavy_minus minus remove
Symbols|➡️ arrow_right next
Symbols|⬅️ arrow_left back previous
Symbols|⬆️ arrow_up up
Symbols|⬇️ arrow_down down
Symbols|↔️ left_right_arrow
Symbols|🔄 arrows_counterclockwise refresh sync repeat
Symbols|🔁 repeat loop
Symbols|🔂 repeat_one
Symbols|🔀 shuffle
Symbols|▶️ play start
Symbols|⏸️ pause waiting
Symbols|⏹️ stop
Symbols|⏩ fast_forward
Symbols|🔴 red_circle
Symbols|🟠 orange_circle
Symbols|🟡 yellow_circle
Symbols|🟢 green_circle ok go
Symbols|🔵 blue_circle
Symbols|🟣 purple_circle
Symbols|⚫ black_circle
Symbols|⚪ white_circle
Symbols|🟥 red_square
Symbols|🟩 green_square
Symbols|🟦 blue_square
Symbols|🔺 red_triangle
Symbols|💬 speech_balloon comment chat
Symbols|💭 thought_balloon thinking
Symbols|🗯️ anger_bubble
Symbols|🏁 checkered_flag finish
Symbols|🚩 triangular_flag red_flag
Symbols|🏳️ white_flag
Symbols|🔗 link chain url
Symbols|🆕 new
Symbols|🆗 ok
Symbols|🔥 hot_fire
Symbols|🆘 sos help
Symbols|©️ copyright
Symbols|™️ tm trademark
`

/** All emojis, in menu order. */
export const EMOJIS: Emoji[] = RAW.split('\n')
  .map((l) => l.trim())
  .filter(Boolean)
  .map((l) => {
    const [group, rest] = l.split('|')
    const [char, name, ...words] = rest.split(' ')
    return { char, name, words: words.join(' '), group: group as EmojiGroup }
  })

export const EMOJI_GROUPS: EmojiGroup[] = [
  'Smileys',
  'People',
  'Animals & nature',
  'Food',
  'Activities',
  'Travel',
  'Objects',
  'Symbols',
]

const norm = (s: string) => s.toLowerCase().replace(/[\s-]+/g, '_')

/** Emojis matching what was typed: shortcode prefix first, then search words, then anywhere in the name. */
export function searchEmoji(query: string, limit = 60): Emoji[] {
  const q = norm(query.trim().replace(/^:/, ''))
  if (!q) return EMOJIS.slice(0, limit)
  const rank = (e: Emoji): number => {
    if (e.name === q) return 0
    if (e.name.startsWith(q)) return 1
    if (e.words.split(' ').some((w) => w.startsWith(q))) return 2
    if (e.name.includes(q)) return 3
    if (e.words.includes(q)) return 4
    return 9
  }
  const seen = new Set<string>()
  return EMOJIS.map((e, i) => ({ e, i, r: rank(e) }))
    .filter((x) => x.r < 9)
    .sort((a, b) => a.r - b.r || a.i - b.i)
    .map((x) => x.e)
    .filter((e) => !seen.has(e.char) && !!seen.add(e.char))
    .slice(0, limit)
}

const RECENT_KEY = 'notatnik-emoji-recent'

/** Emojis picked lately on this device, newest first. */
export function recentEmoji(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]')
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string').slice(0, 16) : []
  } catch {
    return []
  }
}

export function rememberEmoji(char: string): void {
  try {
    localStorage.setItem(
      RECENT_KEY,
      JSON.stringify([char, ...recentEmoji().filter((c) => c !== char)].slice(0, 16)),
    )
  } catch {
    /* not remembered */
  }
}
