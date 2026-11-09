import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_de.dart';
import 'app_localizations_en.dart';
import 'app_localizations_es.dart';
import 'app_localizations_fr.dart';
import 'app_localizations_it.dart';
import 'app_localizations_ja.dart';
import 'app_localizations_pt.dart';
import 'app_localizations_ru.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of L10n
/// returned by `L10n.of(context)`.
///
/// Applications need to include `L10n.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: L10n.localizationsDelegates,
///   supportedLocales: L10n.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the L10n.supportedLocales
/// property.
abstract class L10n {
  L10n(String locale)
    : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static L10n of(BuildContext context) {
    return Localizations.of<L10n>(context, L10n)!;
  }

  static const LocalizationsDelegate<L10n> delegate = _L10nDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
        delegate,
        GlobalMaterialLocalizations.delegate,
        GlobalCupertinoLocalizations.delegate,
        GlobalWidgetsLocalizations.delegate,
      ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('de'),
    Locale('en'),
    Locale('es'),
    Locale('fr'),
    Locale('it'),
    Locale('ja'),
    Locale('pt'),
    Locale('ru'),
    Locale('zh'),
  ];

  /// Button: stop what was about to happen and close the dialog or panel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get commonCancel;

  /// Button: keep the changes made.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get commonSave;

  /// Button or tooltip: close the dialog, panel or view.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get commonClose;

  /// What a screen reader says when a message comes in the chat open on screen: who wrote it and its words (or the name of the file sent). Not shown on screen.
  ///
  /// In en, this message translates to:
  /// **'New message from {name}: {text}'**
  String commonNewMessageFrom(String name, String text);

  /// What a screen reader says when several messages come at once into the chat on screen, all from one person: how many and from whom. Not shown on screen.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 new message from {name}} other{{n} new messages from {name}}}'**
  String commonNewMessagesFrom(int n, String name);

  /// What a screen reader says when several messages from different people come at once into the chat on screen: how many. Not shown on screen.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 new message} other{{n} new messages}}'**
  String commonNewMessages(int n);

  /// Button: say yes to a request or invitation.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get commonAccept;

  /// Button: say no to a request or invitation.
  ///
  /// In en, this message translates to:
  /// **'Decline'**
  String get commonDecline;

  /// Button or tooltip: send the message or files.
  ///
  /// In en, this message translates to:
  /// **'Send'**
  String get commonSend;

  /// Button: try the same thing again after it failed.
  ///
  /// In en, this message translates to:
  /// **'Try again'**
  String get commonTryAgain;

  /// Button or tooltip: copy to the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Copy'**
  String get commonCopy;

  /// Short confirmation shown on a button right after something was copied to the clipboard.
  ///
  /// In en, this message translates to:
  /// **'Copied'**
  String get commonCopied;

  /// Button or tooltip: go back to the previous view.
  ///
  /// In en, this message translates to:
  /// **'Back'**
  String get commonBack;

  /// Button: throw away changes that were not saved.
  ///
  /// In en, this message translates to:
  /// **'Discard'**
  String get commonDiscard;

  /// Button: close the question and go on editing (nothing is lost).
  ///
  /// In en, this message translates to:
  /// **'Keep editing'**
  String get commonKeepEditing;

  /// Button: answer later; the question can come back.
  ///
  /// In en, this message translates to:
  /// **'Not now'**
  String get commonNotNow;

  /// Button or tooltip: fetch a file to this computer.
  ///
  /// In en, this message translates to:
  /// **'Download'**
  String get commonDownload;

  /// The app settings (title, tooltip and button).
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get commonSettings;

  /// The home page of the app (tooltip of the home button in the rail).
  ///
  /// In en, this message translates to:
  /// **'Home'**
  String get commonHome;

  /// The Friends page (list of friends).
  ///
  /// In en, this message translates to:
  /// **'Friends'**
  String get commonFriends;

  /// Button: go on to the next step.
  ///
  /// In en, this message translates to:
  /// **'Continue'**
  String get commonContinue;

  /// Button: finish and close.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get commonDone;

  /// Button: take something out (of a list, a group).
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get commonRemove;

  /// Label for the person using this computer, in lists of people.
  ///
  /// In en, this message translates to:
  /// **'You'**
  String get commonYou;

  /// A moment yesterday: the word for yesterday and a clock time like 23:23 or 11:23 PM. Lower case: it can follow other words. {hour} is "one" when the clock time's hour is 1 (1:05, 01:05, 1:05 PM), else "other", for languages with an article before the time (Spanish "a la 1:05" but "a las 23:05", Portuguese "à 1:05" but "às 23:05", Italian "all’1:05"). English says the same in both cases.
  ///
  /// In en, this message translates to:
  /// **'{hour, select, one{yesterday {time}} other{yesterday {time}}}'**
  String timeYesterdayAt(String time, String hour);

  /// A moment on another day: a date (like "Oct 21" or "Oct 21 2025", already in the language's own format) and a clock time.
  ///
  /// In en, this message translates to:
  /// **'{date}, {time}'**
  String timeDayAt(String date, String time);

  /// How long something has lasted, when it just began (shown where "42 min" would be).
  ///
  /// In en, this message translates to:
  /// **'now'**
  String get timeNow;

  /// A number of minutes, short: how long something lasted or a wait (always at least 1).
  ///
  /// In en, this message translates to:
  /// **'{n} min'**
  String timeMinutes(int n);

  /// A number of hours, short: how long something lasted or a wait.
  ///
  /// In en, this message translates to:
  /// **'{n} h'**
  String timeHours(int n);

  /// Hours and minutes, short: how long something lasted (like "2 h 5 min").
  ///
  /// In en, this message translates to:
  /// **'{hours} h {minutes} min'**
  String timeHoursMinutes(int hours, int minutes);

  /// A number of days: a wait, rounded up (like "2 days").
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 day} other{{n} days}}'**
  String timeDays(int n);

  /// A span chosen by someone, in minutes: "Marta took you out of the room for 5 minutes".
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 minute} other{{n} minutes}}'**
  String spanMinutes(int n);

  /// A span chosen by someone, in hours.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 hour} other{{n} hours}}'**
  String spanHours(int n);

  /// A span chosen by someone, in days.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 day} other{{n} days}}'**
  String spanDays(int n);

  /// A span chosen by someone, in weeks.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 week} other{{n} weeks}}'**
  String spanWeeks(int n);

  /// How long ago: less than a minute. Lower case: it follows words like "Seen".
  ///
  /// In en, this message translates to:
  /// **'a moment ago'**
  String get agoMoment;

  /// How long ago, in minutes (1 to 59). Lower case: it follows words like "Seen".
  ///
  /// In en, this message translates to:
  /// **'{n} min ago'**
  String agoMinutes(int n);

  /// How long ago, in hours, earlier today.
  ///
  /// In en, this message translates to:
  /// **'{n} h ago'**
  String agoHours(int n);

  /// How long ago: yesterday. Lower case: it follows words like "Seen".
  ///
  /// In en, this message translates to:
  /// **'yesterday'**
  String get agoYesterday;

  /// How long ago, in days (2 to 6).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, other{{n} days ago}}'**
  String agoDays(int n);

  /// How long ago, in weeks (1 to 4).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 week ago} other{{n} weeks ago}}'**
  String agoWeeks(int n);

  /// How long ago, in months (1 to 11).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 month ago} other{{n} months ago}}'**
  String agoMonths(int n);

  /// How long ago: more than a year.
  ///
  /// In en, this message translates to:
  /// **'over a year ago'**
  String get agoOverYear;

  /// A file size in bytes (under 1000).
  ///
  /// In en, this message translates to:
  /// **'{n} B'**
  String sizeBytes(int n);

  /// A file size in kilobytes; {n} is already formatted, like 48.2.
  ///
  /// In en, this message translates to:
  /// **'{n} KB'**
  String sizeKB(String n);

  /// A file size in megabytes; {n} is already formatted, like 48.2.
  ///
  /// In en, this message translates to:
  /// **'{n} MB'**
  String sizeMB(String n);

  /// A file size in gigabytes; {n} is already formatted, like 1.5.
  ///
  /// In en, this message translates to:
  /// **'{n} GB'**
  String sizeGB(String n);

  /// A file size in terabytes; {n} is already formatted.
  ///
  /// In en, this message translates to:
  /// **'{n} TB'**
  String sizeTB(String n);

  /// Two names in a row: "Marta and Leo".
  ///
  /// In en, this message translates to:
  /// **'{first} and {second}'**
  String listTwo(String first, String second);

  /// Three or more names: {rest} is all but the last joined with the list separator ("Marta, Leo"), then the last one ("Sofía").
  ///
  /// In en, this message translates to:
  /// **'{rest} and {last}'**
  String listMore(String rest, String last);

  /// What goes between names in a list of three or more, before the last one: "Marta, Leo and Sofía". Keep the space if the language uses one.
  ///
  /// In en, this message translates to:
  /// **', '**
  String get listSeparator;

  /// A place in a list counted from 1: a member's place in a group ("3rd in this group"). English computes its own endings in code; other languages write their ordinal here (for example "{n}.º", "{n}.", "{n}番目").
  ///
  /// In en, this message translates to:
  /// **'No. {n}'**
  String ordinal(int n);

  /// Error message. An invite or friend link is too old.
  ///
  /// In en, this message translates to:
  /// **'The link has expired. Ask for a new one.'**
  String get errLinkExpired;

  /// Error message. A group invitation link was pasted where a friend link was expected.
  ///
  /// In en, this message translates to:
  /// **'That is a group invitation.'**
  String get errGroupInvite;

  /// Error message. The friend's computer could not be reached.
  ///
  /// In en, this message translates to:
  /// **'The other device was not found. Check that you are both online.'**
  String get errDeviceNotFound;

  /// Error message. What was pasted or typed is not a friend link or code.
  ///
  /// In en, this message translates to:
  /// **'That does not look like a Fogata link or code.'**
  String get errNotFriendCode;

  /// Error message. Joining a group this person is already a member of.
  ///
  /// In en, this message translates to:
  /// **'You are already in that group.'**
  String get errAlreadyInGroup;

  /// Error message. Something needs the person to be in the voice room first.
  ///
  /// In en, this message translates to:
  /// **'Enter the room first.'**
  String get errJoinRoomFirst;

  /// Error message. The profile on disk comes from a newer app version.
  ///
  /// In en, this message translates to:
  /// **'Your profile was saved by a newer version of Fogata. Update Fogata to open it.'**
  String get errNewerVersion;

  /// Error message. The system keychain (password store) is locked or refused access.
  ///
  /// In en, this message translates to:
  /// **'Fogata could not get the key to your profile from this computer\'s keychain. Unlock the keychain or allow Fogata to use it, then try again.'**
  String get errKeychainUnreachable;

  /// Error message. The key that opens the profile is gone from the system keychain.
  ///
  /// In en, this message translates to:
  /// **'This computer\'s keychain no longer holds the key to your profile, so Fogata cannot open it.'**
  String get errKeychainNoKey;

  /// Error message. The key in the system keychain is not the right one.
  ///
  /// In en, this message translates to:
  /// **'The key in this computer\'s keychain does not open your profile.'**
  String get errKeychainWrongKey;

  /// Error message. The group's encryption key has not arrived yet.
  ///
  /// In en, this message translates to:
  /// **'This group is not ready on this computer yet. Try again in a moment.'**
  String get errNoGroupKey;

  /// Error message. Another copy of the app uses the same profile.
  ///
  /// In en, this message translates to:
  /// **'Fogata is already open with this profile. Close the other one first.'**
  String get errAlreadyRunning;

  /// Error message. A shared file was taken out of the room.
  ///
  /// In en, this message translates to:
  /// **'That file is no longer in the room.'**
  String get errFileGone;

  /// Error message. Nobody who has the file is connected.
  ///
  /// In en, this message translates to:
  /// **'Nobody online has that file right now.'**
  String get errNobodyHasFile;

  /// Error message. The app is not ready yet.
  ///
  /// In en, this message translates to:
  /// **'Fogata is still starting.'**
  String get errStillStarting;

  /// Error message. Trying to make a second identity on a computer that has one.
  ///
  /// In en, this message translates to:
  /// **'This computer already has an identity; its key cannot change.'**
  String get errIdentityExists;

  /// Error message. A name was empty, too long or made only of invisible characters.
  ///
  /// In en, this message translates to:
  /// **'Write a name of 1 to 40 visible characters.'**
  String get errNameRule;

  /// Error message. The profile bio breaks the limits.
  ///
  /// In en, this message translates to:
  /// **'A bio has at most 190 characters on 4 lines, and no hidden characters.'**
  String get errBioRule;

  /// Error message. A profile or group picture is too large or of the wrong type.
  ///
  /// In en, this message translates to:
  /// **'The picture must be a PNG or JPEG of 64 KB at most.'**
  String get errPictureRule;

  /// Error message. What was pasted is not a friend link.
  ///
  /// In en, this message translates to:
  /// **'That is not a Fogata friend link.'**
  String get errNotFriendLink;

  /// Error message. A friend link was cut or changed.
  ///
  /// In en, this message translates to:
  /// **'The link is incomplete or damaged.'**
  String get errLinkDamaged;

  /// Error message. The link was made by a newer app version.
  ///
  /// In en, this message translates to:
  /// **'That link needs a newer version of Fogata.'**
  String get errLinkNeedsNewer;

  /// Error message. A friend code was typed in the wrong shape. Keep FGT-XXXXX-XXXXX as is.
  ///
  /// In en, this message translates to:
  /// **'A code looks like FGT-XXXXX-XXXXX.'**
  String get errCodeShape;

  /// Error message. A friend code does not match or expired.
  ///
  /// In en, this message translates to:
  /// **'The code is wrong or no longer works.'**
  String get errCodeWrong;

  /// Error message. The other computer failed the identity check.
  ///
  /// In en, this message translates to:
  /// **'The other device could not prove who it is.'**
  String get errNotProven;

  /// Error message. Answering a call that already stopped ringing.
  ///
  /// In en, this message translates to:
  /// **'That call is no longer ringing.'**
  String get errCallGone;

  /// Error message. Starting a call while in another one.
  ///
  /// In en, this message translates to:
  /// **'You are already in a call. Hang up first.'**
  String get errAlreadyInCall;

  /// Error message. Adding someone who is already a friend.
  ///
  /// In en, this message translates to:
  /// **'You are already friends.'**
  String get errAlreadyFriends;

  /// Error: a friend request sent from a group member's card could not go, because that member is not connected now.
  ///
  /// In en, this message translates to:
  /// **'That member is not connected right now. Try again when they are online.'**
  String get errMemberNotConnected;

  /// Error: a friend request sent from a group member's card could not go, because that member runs an older version of Fogata. A friend link is a link made in Friends to add a friend.
  ///
  /// In en, this message translates to:
  /// **'That member’s copy of Fogata is too old to take friend requests from a group. Send them a friend link instead.'**
  String get errMemberTooOld;

  /// Error: a friend request sent from a group member's card was lost on the way (the connection dropped, say).
  ///
  /// In en, this message translates to:
  /// **'The friend request did not reach that member. Try again.'**
  String get errMemberAskLost;

  /// Error message. The person pasted their own friend link.
  ///
  /// In en, this message translates to:
  /// **'That link is your own: send it to the friend you want to add.'**
  String get errOwnLink;

  /// Error message. A one-time friend link was used before or is too old.
  ///
  /// In en, this message translates to:
  /// **'That link was already used or has expired. Ask for a new one.'**
  String get errLinkUsed;

  /// Error message. The link ran out while looking for the friend's computer.
  ///
  /// In en, this message translates to:
  /// **'The link expired before its device could be reached. Ask for a new one.'**
  String get errLinkExpiredUnreached;

  /// Error message. Nobody answered a typed friend code.
  ///
  /// In en, this message translates to:
  /// **'No device answered the code. Check it, and that the other device still shows it.'**
  String get errCodeUnanswered;

  /// Error message. The other computer refused the friend request.
  ///
  /// In en, this message translates to:
  /// **'The other device said no. The code may have been used already.'**
  String get errRefused;

  /// Error message. Joining a group with an invite, but no member answered.
  ///
  /// In en, this message translates to:
  /// **'Nobody let you in. Try again when someone in the group is online.'**
  String get errNobodyLetIn;

  /// Error message. A manager acted on someone at or above their own place.
  ///
  /// In en, this message translates to:
  /// **'You can only do that to someone below you in the group.'**
  String get errNotBelow;

  /// Error message. Someone who does not manage the group tried a manager action.
  ///
  /// In en, this message translates to:
  /// **'Only those who manage the group can do that.'**
  String get errNotManager;

  /// Error message. Removing one's own manager role in a way that is not allowed.
  ///
  /// In en, this message translates to:
  /// **'You can’t take yourself off the group’s managers.'**
  String get errLastManager;

  /// Error message. Deleting a voice room that has people in it.
  ///
  /// In en, this message translates to:
  /// **'People are in that voice room: remove it once it is empty.'**
  String get errRoomNotEmpty;

  /// Error message. Sharing more files than the room allows per person.
  ///
  /// In en, this message translates to:
  /// **'You already share the most files one person can in a room. Stop sharing some first.'**
  String get errTooManyFiles;

  /// Error message. The computer's disk has no free space.
  ///
  /// In en, this message translates to:
  /// **'The disk is full. Free some space and try again.'**
  String get errDiskFull;

  /// Error message. A limit on changes to a group by one device.
  ///
  /// In en, this message translates to:
  /// **'This device has made too many changes to the group that no other member has taken in yet. Try again after someone else in the group makes a change.'**
  String get errTooManyChanges;

  /// Error message. An error with no explanation.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong.'**
  String get errSomething;

  /// Error message: a voice room has no room for more people.
  ///
  /// In en, this message translates to:
  /// **'That voice room is full: it takes {n} people at most.'**
  String errRoomFull(int n);

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'No peer of the public network answered: this computer does not seem to reach the internet, or a firewall blocks Fogata.'**
  String get findingOffline;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'UDP works. Calls and getting through routers need it.'**
  String get findingUdpWorks;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'This network or this computer seems to block UDP (peers that answered over TCP did not over QUIC). Chat can still go through other members, but direct connections through routers and calls will mostly fail. Check the firewall or the network rules.'**
  String get findingUdpBlocked;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'This computer has a carrier-grade NAT address (100.64.0.0/10): the provider shares one public address among many customers.'**
  String get findingCarrierNat;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'This computer has a public IPv4 address: others can reach it directly.'**
  String get findingPublicIpv4;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'Behind a router that gives a different public port to each destination (a hard NAT).'**
  String get findingHardNat;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'Behind a router that keeps the same public port for every destination (an easy NAT): it can be opened from both sides at once.'**
  String get findingEasyNat;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'Too few peers have seen this computer’s public address to tell what kind of router there is.'**
  String get findingUnknownRouter;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'No public IPv6 address. Fogata does not need it, but it makes direct connections easier.'**
  String get findingNoIpv6;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'Public IPv6 works: friends with IPv6 connect with no router in the way.'**
  String get findingIpv6Works;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'This computer has a public IPv6 address, but no peer has reached it over IPv6 yet.'**
  String get findingIpv6Unreached;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'In short: expect trouble with direct connections until UDP is allowed.'**
  String get findingSummaryNoUdp;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'In short: your friends should be able to reach this computer directly.'**
  String get findingSummaryPublic;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'In short: direct connections should work with most of your friends, opening both routers at once; with a friend behind a hard NAT, a group member in between is needed.'**
  String get findingSummaryEasyNat;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'In short: friends with IPv6 or a public address connect directly; with the others, a group member in between is needed.'**
  String get findingSummaryHardNatWithIpv6;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'In short: only friends with a public address connect directly; with the others, a group member in between is needed.'**
  String get findingSummaryHardNat;

  /// A finding of the network check in Settings › Network. Technical terms (UDP, TCP, NAT, IPv4, IPv6, peer, router) may stay as they are usually written in the language.
  ///
  /// In en, this message translates to:
  /// **'In short: not enough information; check again in a minute.'**
  String get findingSummaryUnknown;

  /// A finding of the network check: the internet works; {n} computers of the public network are known.
  ///
  /// In en, this message translates to:
  /// **'Internet: this computer reaches the public network ({n} peers known).'**
  String findingInternet(int n);

  /// A finding of the network check: the DNS settings could not be read; {detail} is technical text in English from the system.
  ///
  /// In en, this message translates to:
  /// **'DNS: the system settings could not be read as a whole ({detail})'**
  String findingDns(String detail);

  /// A finding of the network check: other computers running Fogata on the same local network.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 Fogata device} other{{n} Fogata devices}} on this local network: friends here connect without going through the internet.'**
  String findingLanDevices(int n);

  /// A finding of the network check, followed by a command to copy. {system} picks where to run it: windows (PowerShell as administrator), macos (the Terminal app) or other (a terminal on Linux).
  ///
  /// In en, this message translates to:
  /// **'This computer’s firewall lets nothing in for Fogata, so a friend in the same place behind another router (two routers at home, for example) cannot reach it. Run this command once {system, select, windows{in PowerShell as administrator} macos{in Terminal} other{in a terminal}} to let Fogata in:'**
  String findingFirewall(String system);

  /// Settings › Language & time: title of the language choice.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get settingsLanguage;

  /// Settings › Language & time: explains the language choice; messages from people are not translated.
  ///
  /// In en, this message translates to:
  /// **'The language of Fogata’s own words. Messages stay as they were written.'**
  String get settingsLanguageNote;

  /// Settings › Language & time: label of the language list (read by screen readers).
  ///
  /// In en, this message translates to:
  /// **'Choose the language'**
  String get settingsLanguageChoose;

  /// Settings › Language & time: title of the clock format choice.
  ///
  /// In en, this message translates to:
  /// **'Time'**
  String get settingsTime;

  /// Settings › Language & time: explains the clock format choice.
  ///
  /// In en, this message translates to:
  /// **'How times of day show, in the chat, calls and lists.'**
  String get settingsTimeNote;

  /// Settings › Language & time: the 24-hour clock option, with an example time (like 14:52).
  ///
  /// In en, this message translates to:
  /// **'24 hours · {example}'**
  String settingsTime24(String example);

  /// Settings › Language & time: the 12-hour clock option, with an example time (like 2:52 PM).
  ///
  /// In en, this message translates to:
  /// **'12 hours · {example}'**
  String settingsTime12(String example);

  /// Bell notice about the firewall, followed by a command line {command} to run (keep it as is). {system} picks where to run it: windows (PowerShell as administrator), macos (the Terminal app) or other (a terminal on Linux).
  ///
  /// In en, this message translates to:
  /// **'A friend in the same place behind another router cannot reach this computer. Run once {system, select, windows{in PowerShell as administrator} macos{in Terminal} other{in a terminal}}: {command}'**
  String firewallNoticeText(String system, String command);

  /// Short note shown after the firewall command was copied. {system} picks where to paste it: windows (PowerShell as administrator), macos (the Terminal app) or other (a terminal on Linux).
  ///
  /// In en, this message translates to:
  /// **'Copied. Paste it {system, select, windows{in PowerShell as administrator} macos{in Terminal} other{in a terminal}} and run it.'**
  String firewallCopied(String system);

  /// Toast when the app restarted itself after a crash.
  ///
  /// In en, this message translates to:
  /// **'Fogata stopped working and started again.'**
  String get appRestarted;

  /// A voice room named with its group, when the group has several voice rooms: "General in Friday games". Used inside other sentences, like "You’re both in {room}".
  ///
  /// In en, this message translates to:
  /// **'{room} in {group}'**
  String appVoiceRoomInGroup(String room, String group);

  /// Toast when calling a friend who is already in the same voice room as you. {room} is a voice room name, or "General in Friday games".
  ///
  /// In en, this message translates to:
  /// **'You’re both in {room}.'**
  String appBothIn(String room);

  /// Toast: the friend in a direct call ended it. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'{name} hung up'**
  String appHungUp(String name);

  /// Stands for a friend whose name is not known, as {name} in "{name} hung up".
  ///
  /// In en, this message translates to:
  /// **'Your friend'**
  String get appYourFriend;

  /// Toast: a direct call ended because the other person left and did not come back.
  ///
  /// In en, this message translates to:
  /// **'Nobody came back, so the call ended'**
  String get appCallEndedAlone;

  /// Toast: a ringing call was answered on another computer with the same identity.
  ///
  /// In en, this message translates to:
  /// **'Answered on another computer'**
  String get appAnsweredElsewhere;

  /// Toast: the voice room being shown was deleted by a manager. {room} is its name.
  ///
  /// In en, this message translates to:
  /// **'“{room}” was deleted.'**
  String appVoiceRoomDeleted(String room);

  /// Stands for a person whose name is not known (start of a sentence or alone).
  ///
  /// In en, this message translates to:
  /// **'Someone'**
  String get appSomeone;

  /// Stands for a group whose name is not known, inside sentences like "Marta asks to join “a group”". Lower case.
  ///
  /// In en, this message translates to:
  /// **'a group'**
  String get appAGroup;

  /// Toast: someone sent a friend request. {name} is their name.
  ///
  /// In en, this message translates to:
  /// **'{name} wants to be your friend.'**
  String appFriendRequest(String name);

  /// Toast: a friend request reached the other person. {name} is their name.
  ///
  /// In en, this message translates to:
  /// **'Request delivered to {name}. It waits for their answer.'**
  String appRequestDelivered(String name);

  /// Toast: a friend request was accepted. {name} is the new friend.
  ///
  /// In en, this message translates to:
  /// **'You and {name} are friends now.'**
  String appFriendsNow(String name);

  /// Toast: this person joined a group. {group} is its name.
  ///
  /// In en, this message translates to:
  /// **'You are in “{group}” now.'**
  String appJoinedGroup(String group);

  /// Toast: this person was removed from a group. {group} is its name.
  ///
  /// In en, this message translates to:
  /// **'You are no longer in “{group}”.'**
  String appRemovedFromGroup(String group);

  /// Toast: a friend added this person to a group; it waits as an invitation. {name} is the friend, {group} the group.
  ///
  /// In en, this message translates to:
  /// **'{name} added you to “{group}”. Join it or not: your call.'**
  String appAddedToGroup(String name, String group);

  /// Toast for managers: someone used an invite link and waits to be let in. {name} is that person, {group} the group.
  ///
  /// In en, this message translates to:
  /// **'{name} asks to join “{group}”.'**
  String appAsksToJoin(String name, String group);

  /// Toast: an invite link was used; a member must let this person in. {group} is the group.
  ///
  /// In en, this message translates to:
  /// **'Waiting for someone in “{group}” to let you in.'**
  String appJoinWaiting(String group);

  /// Toast: a member refused to let this person into a group. {group} is the group.
  ///
  /// In en, this message translates to:
  /// **'Someone in “{group}” said no. That invitation no longer works.'**
  String appJoinDeclined(String group);

  /// Toast: a file was downloaded; {path} is where it was saved on this computer.
  ///
  /// In en, this message translates to:
  /// **'Downloaded to {path}'**
  String appDownloadedTo(String path);

  /// Toast: a download failed; {why} is the reason, a full sentence.
  ///
  /// In en, this message translates to:
  /// **'Could not download: {why}'**
  String appDownloadFailed(String why);

  /// Toast and bell notice: the same identity runs on two computers at once.
  ///
  /// In en, this message translates to:
  /// **'This identity is open on another computer too. Close Fogata there: one identity works on one computer at a time.'**
  String get appOtherSession;

  /// Toast at start: the friends list file was damaged and an earlier copy was used.
  ///
  /// In en, this message translates to:
  /// **'Your list of friends was damaged on this computer and was restored from the copy made before its last change. A friend added or removed just before may need adding or removing again.'**
  String get appFriendsRestored;

  /// Toast: an invite link expired while waiting to be let in. {group} is the group.
  ///
  /// In en, this message translates to:
  /// **'The invitation to “{group}” ran out before someone in the group let you in. Ask for a new one.'**
  String appInvitationRanOut(String group);

  /// Toast: someone who manages the group took this person out of a voice room. {who} is their name, {room} the voice room.
  ///
  /// In en, this message translates to:
  /// **'{who} took you out of {room}.'**
  String appTakenOut(String who, String room);

  /// Toast: a voice room was limited to chosen members, and this person is not one. {room} is the voice room.
  ///
  /// In en, this message translates to:
  /// **'{room} is for some members only now.'**
  String appRoomForSome(String room);

  /// Toast: taken out of a voice room until someone higher in the group lets them back. {who} is a name, {room} the voice room.
  ///
  /// In en, this message translates to:
  /// **'{who} took you out of {room} until someone above you lets you back.'**
  String appTakenOutUntilLetBack(String who, String room);

  /// Toast: taken out of a voice room for a time. {who} is a name, {room} the voice room, {span} how long, like "5 minutes" or "1 day".
  ///
  /// In en, this message translates to:
  /// **'{who} took you out of {room} for {span}.'**
  String appTakenOutFor(String who, String room, String span);

  /// Toast: taken out of a voice room for a time that is not known. {who} is a name, {room} the voice room.
  ///
  /// In en, this message translates to:
  /// **'{who} took you out of {room} for a while.'**
  String appTakenOutWhile(String who, String room);

  /// Why this person cannot enter a voice room yet: they were timed out. {room} is the voice room, {wait} how long is left, like "58 min", "3 h" or "2 days".
  ///
  /// In en, this message translates to:
  /// **'You can come back to {room} in {wait}.'**
  String appComeBackIn(String room, String wait);

  /// Why this person cannot enter a voice room now. {room} is the voice room.
  ///
  /// In en, this message translates to:
  /// **'{room} is not for you now.'**
  String appRoomNotForYou(String room);

  /// Toast: folders were dropped on a room; only files can be shared. {n} is how many folders were left out.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{Folders can’t be shared, only files: the folder was left out.} other{Folders can’t be shared, only files: {n} folders were left out.}}'**
  String appFoldersNotShared(int n);

  /// Toast: one file is now shared in the room. {file} is its name.
  ///
  /// In en, this message translates to:
  /// **'Sharing {file} from this computer. It stops being available when you leave the room.'**
  String appSharingOne(String file);

  /// Toast: several files are now shared in the room. {n} is how many (2 or more).
  ///
  /// In en, this message translates to:
  /// **'Sharing {n} files from this computer. They stop being available when you leave the room.'**
  String appSharingMany(int n);

  /// Toast: a file or picture was saved; {path} is where, on this computer.
  ///
  /// In en, this message translates to:
  /// **'Saved to {path}'**
  String appSavedTo(String path);

  /// Toast: an invite link was used; a member must let this person in. {group} is the group.
  ///
  /// In en, this message translates to:
  /// **'Asking to join “{group}”. Someone in the group lets you in when they see it.'**
  String appAskingToJoin(String group);

  /// Toast: a friend link or code was used; Fogata looks for the friend's computer.
  ///
  /// In en, this message translates to:
  /// **'Looking for the other device to send your request…'**
  String get appLookingForDevice;

  /// Why a direct call did not go through (title and toast). {name} is the friend called.
  ///
  /// In en, this message translates to:
  /// **'{name} didn’t answer'**
  String callStateNoAnswer(String name);

  /// Why a direct call did not go through: the friend declined. {name} is the friend.
  ///
  /// In en, this message translates to:
  /// **'{name} can’t talk right now'**
  String callStateDeclined(String name);

  /// Why a direct call did not go through: the friend is busy. {name} is the friend.
  ///
  /// In en, this message translates to:
  /// **'{name} is in another call'**
  String callStateBusy(String name);

  /// Why a direct call did not go through: the friend's app is too old. {name} is the friend.
  ///
  /// In en, this message translates to:
  /// **'{name} needs a newer Fogata to join'**
  String callStateOutdated(String name);

  /// Why a direct call did not go through: the friend's computer was not reached. {name} is the friend.
  ///
  /// In en, this message translates to:
  /// **'{name} couldn’t be reached'**
  String callStateUnreached(String name);

  /// Button: go back into a direct call this person left while the friend is still in it. Short.
  ///
  /// In en, this message translates to:
  /// **'Return to call'**
  String get callStateReturn;

  /// Button: call a friend back after a missed call. Short.
  ///
  /// In en, this message translates to:
  /// **'Call back'**
  String get callStateCallBack;

  /// Button: start a direct (voice) call with a friend. Short.
  ///
  /// In en, this message translates to:
  /// **'Call'**
  String get callStateCall;

  /// Bell notice title: a person this device invited to a group said no. {name} is the person, {group} the group.
  ///
  /// In en, this message translates to:
  /// **'{name} declined the invitation to “{group}”'**
  String noticesDeclined(String name, String group);

  /// Toast: a person this device invited to a group said no. {name} is the person, {group} the group.
  ///
  /// In en, this message translates to:
  /// **'{name} declined the invitation to “{group}”.'**
  String noticesDeclinedToast(String name, String group);

  /// Bell notice and corner note: a friend called while this person was in another call. {name} is the friend.
  ///
  /// In en, this message translates to:
  /// **'{name} called while you were in a call'**
  String noticesMissedInCall(String name);

  /// Bell notice and corner note: a friend called while this person appeared offline. {name} is the friend.
  ///
  /// In en, this message translates to:
  /// **'{name} called while you appeared offline'**
  String noticesMissedOffline(String name);

  /// Bell notice and corner note: a missed direct call. {name} is the friend who called.
  ///
  /// In en, this message translates to:
  /// **'Missed call from {name}'**
  String noticesMissedCall(String name);

  /// The name of a group's first voice room until someone renames it. Short.
  ///
  /// In en, this message translates to:
  /// **'General'**
  String get contactsGeneral;

  /// Disappearing messages timer of a direct chat: off (messages stay). Short: fits a small pill.
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get directStateTimerOff;

  /// Disappearing messages timer of a direct chat: messages go after 24 hours. Short: fits a small pill.
  ///
  /// In en, this message translates to:
  /// **'24 hours'**
  String get directStateTimer24h;

  /// Disappearing messages timer of a direct chat: messages go after {n} days (7 or 90). Short: fits a small pill.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 day} other{{n} days}}'**
  String directStateTimerDays(int n);

  /// Tooltip on a direct message that will disappear: minutes left (1 to 59).
  ///
  /// In en, this message translates to:
  /// **'Disappears in {n} min'**
  String directStateDisappearsMinutes(int n);

  /// Tooltip on a direct message that will disappear: hours left (1 to 47).
  ///
  /// In en, this message translates to:
  /// **'Disappears in {n} h'**
  String directStateDisappearsHours(int n);

  /// Tooltip on a direct message that will disappear: days left (2 or more).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, other{Disappears in {n} days}}'**
  String directStateDisappearsDays(int n);

  /// Settings › Storage: the room for direct messages' pictures, videos and files has no limit. Short: fits a list.
  ///
  /// In en, this message translates to:
  /// **'No limit'**
  String get directStateNoLimit;

  /// Toast: jumping to a pinned or quoted direct message that was deleted or disappeared.
  ///
  /// In en, this message translates to:
  /// **'That message is no longer here.'**
  String get directStateMessageGone;

  /// Toast: folders were dropped on a direct chat; only files can be sent. {n} is how many folders were left out.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{Folders can’t be sent, only files: the folder was left out.} other{Folders can’t be sent, only files: {n} folders were left out.}}'**
  String directStateFoldersNotSent(int n);

  /// Toast: a file in a direct chat could not be fetched; {why} is the reason, a full sentence.
  ///
  /// In en, this message translates to:
  /// **'Could not get the file: {why}'**
  String directStateFileFailed(String why);

  /// Error when an animated profile picture (what = pictures) or banner (what = banners) has too many pixels to play. {most} and {size} are sizes in pixels like "512 × 512".
  ///
  /// In en, this message translates to:
  /// **'{what, select, banners{Animated banners can be up to {most} pixels; this one is {size}.} other{Animated pictures can be up to {most} pixels; this one is {size}.}}'**
  String movingTooLarge(String what, String most, String size);

  /// Error when an animated profile picture (what = pictures) or banner (what = banners) has too many frames. {most} and {count} are numbers of frames.
  ///
  /// In en, this message translates to:
  /// **'{what, select, banners{Animated banners can have up to {most} frames; this one has {count}.} other{Animated pictures can have up to {most} frames; this one has {count}.}}'**
  String movingTooLong(String what, String most, String count);

  /// Error when an animated profile picture (what = pictures) or banner (what = banners) file is too big. {most} and {size} are file sizes like "550 KB".
  ///
  /// In en, this message translates to:
  /// **'{what, select, banners{Animated banners can be up to {most}; this one is {size}.} other{Animated pictures can be up to {most}; this one is {size}.}}'**
  String movingTooHeavy(String what, String most, String size);

  /// Error under a name field: the name has invisible characters (copied from somewhere).
  ///
  /// In en, this message translates to:
  /// **'This name has hidden characters. Type it again without them.'**
  String get namesHidden;

  /// Error under a name field: the name is made only of blank characters.
  ///
  /// In en, this message translates to:
  /// **'Nothing in this name shows. Use letters, numbers or emoji.'**
  String get namesBlank;

  /// What a profile card shows for someone who never wrote a bio, written as if by them. Friendly and short.
  ///
  /// In en, this message translates to:
  /// **'Hey there! I’m on Fogata.'**
  String get namesDefaultBio;

  /// Error under the bio field: it has invisible characters.
  ///
  /// In en, this message translates to:
  /// **'This bio has hidden characters. Type it again without them.'**
  String get namesBioHidden;

  /// Error under the bio field: too many lines. {n} is the most (4).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{A bio has 1 line at most.} other{A bio has {n} lines at most.}}'**
  String namesBioLines(int n);

  /// Error under the bio field: too long. {n} is the most (190).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{A bio has 1 character at most.} other{A bio has {n} characters at most.}}'**
  String namesBioChars(int n);

  /// Error: the app tried to restart itself and could not.
  ///
  /// In en, this message translates to:
  /// **'Fogata could not start again. Quit it and open it yourself.'**
  String get restartFailed;

  /// Under the message box: one person is typing. {name} is their name (drawn in bold).
  ///
  /// In en, this message translates to:
  /// **'{name} is typing…'**
  String typingOne(String name);

  /// Under the message box: two people are typing. {first} and {second} are their names (drawn in bold).
  ///
  /// In en, this message translates to:
  /// **'{first} and {second} are typing…'**
  String typingTwo(String first, String second);

  /// Under the message box: three or more people are typing.
  ///
  /// In en, this message translates to:
  /// **'Several people are typing…'**
  String get typingSeveral;

  /// Stands for a room whose group name is not known, in "Drop to share in {group}". Lower case.
  ///
  /// In en, this message translates to:
  /// **'the room'**
  String get outgoingTheRoom;

  /// Big title over a room while files are dragged over it. {group} is the group's name.
  ///
  /// In en, this message translates to:
  /// **'Drop to share in {group}'**
  String outgoingDropTitle(String group);

  /// Under the drop title while files are dragged over a room: what happens to the files.
  ///
  /// In en, this message translates to:
  /// **'You can add a message and check them before they go. The others download them from this computer while you are in the room.'**
  String get outgoingDropNote;

  /// Error toast: files dragged onto a room were left out because they are hidden (their name starts with a dot) or in a private place such as a keychain, a browser's profile or the system's folders; a web page can start a drag that names such a file. {n} is how many. "the paperclip" is the room's button that picks files to share.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{A hidden or private file was left out: files like it are not added by dragging, since a web page can start a drag. To share it, pick it with the paperclip.} other{{n} hidden or private files were left out: files like these are not added by dragging, since a web page can start a drag. To share them, pick them with the paperclip.}}'**
  String outgoingPrivateLeftOut(int n);

  /// Above the message box: files waiting to be shared. {n} is how many; {count} is that number again, drawn in its own style; {size} is their total size, like "48.2 MB". Keep the " · " separator before the size.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{{count} file to share · {size}} other{{count} files to share · {size}}}'**
  String outgoingToShare(int n, String count, String size);

  /// Small button above the message box: take out all files waiting to be shared.
  ///
  /// In en, this message translates to:
  /// **'Remove all'**
  String get outgoingRemoveAll;

  /// Tooltip of the button that takes one waiting file out. {file} is the file name.
  ///
  /// In en, this message translates to:
  /// **'Remove {file}'**
  String outgoingRemoveFile(String file);

  /// Menu item of the icon in the system tray / menu bar: show the window.
  ///
  /// In en, this message translates to:
  /// **'Open Fogata'**
  String get windowTrayOpen;

  /// Menu item of the icon in the system tray / menu bar: quit the app.
  ///
  /// In en, this message translates to:
  /// **'Quit Fogata'**
  String get windowTrayQuit;

  /// Tooltip of the window's minimize button in the title bar (Windows and Linux).
  ///
  /// In en, this message translates to:
  /// **'Minimize'**
  String get windowMinimize;

  /// Tooltip of the window's button that brings a maximized window back to its size.
  ///
  /// In en, this message translates to:
  /// **'Restore'**
  String get windowRestore;

  /// Tooltip of the window's maximize button in the title bar.
  ///
  /// In en, this message translates to:
  /// **'Maximize'**
  String get windowMaximize;

  /// Linux (Flatpak): the reason shown by the system when Fogata asks to open at sign-in.
  ///
  /// In en, this message translates to:
  /// **'Open Fogata when you sign in.'**
  String get autostartReason;

  /// Settings › Voice & video: label in front of the microphone choice. Short: fits a narrow label column.
  ///
  /// In en, this message translates to:
  /// **'Microphone'**
  String get devicesMicrophone;

  /// Settings › Voice & video: label in front of the speaker (sound output) choice. Short: fits a narrow label column.
  ///
  /// In en, this message translates to:
  /// **'Speaker'**
  String get devicesSpeaker;

  /// Settings › Voice & video: label in front of the camera choice; also the heading of the camera panel. Short.
  ///
  /// In en, this message translates to:
  /// **'Camera'**
  String get devicesCamera;

  /// Tooltip and screen-reader label of the button that opens the list of microphones.
  ///
  /// In en, this message translates to:
  /// **'Choose microphone'**
  String get devicesChooseMicrophone;

  /// Tooltip and screen-reader label of the button that opens the list of speakers (sound outputs).
  ///
  /// In en, this message translates to:
  /// **'Choose speaker'**
  String get devicesChooseSpeaker;

  /// Tooltip and screen-reader label of the button that opens the list of cameras.
  ///
  /// In en, this message translates to:
  /// **'Choose camera'**
  String get devicesChooseCamera;

  /// Note where a camera would be chosen or tested: this computer has no camera.
  ///
  /// In en, this message translates to:
  /// **'No camera is connected.'**
  String get devicesNoCamera;

  /// Settings › Voice & video › Test video: the camera cannot be tested because the call uses it.
  ///
  /// In en, this message translates to:
  /// **'The camera is on in your call.'**
  String get devicesCameraInCall;

  /// Note or error: the camera could not be opened. On macOS ({system} = macos) it says where to allow it; keep "System Settings › Privacy & Security" as macOS names it in the language.
  ///
  /// In en, this message translates to:
  /// **'{system, select, macos{Could not use the camera. Allow it in System Settings › Privacy & Security.} other{Could not use the camera.}}'**
  String devicesCameraFailed(String system);

  /// Note or error: the system (on Linux, its camera portal: the person, now or before, or the system settings) does not let the app use the camera. Says to allow it in the system settings.
  ///
  /// In en, this message translates to:
  /// **'The system does not let Fogata use the camera. Allow it in the system settings, then try again.'**
  String get devicesCameraDenied;

  /// Note or error on Linux: the app reaches cameras through the system's camera portal (xdg-desktop-portal), which this system lacks. "Camera portal" may stay technical.
  ///
  /// In en, this message translates to:
  /// **'Could not reach the camera: this system has no camera portal.'**
  String get devicesCameraUnavailable;

  /// Settings › Voice & video › Test video: the camera preview is only shown on this computer.
  ///
  /// In en, this message translates to:
  /// **'Only you see the preview.'**
  String get devicesPreviewOnlyYou;

  /// Settings › Voice & video: button that stops the camera test preview.
  ///
  /// In en, this message translates to:
  /// **'Stop video'**
  String get devicesStopVideo;

  /// Settings › Voice & video: button that opens the camera to preview it.
  ///
  /// In en, this message translates to:
  /// **'Test video'**
  String get devicesTestVideo;

  /// Screen-reader label of the noise suppression switch while it is on.
  ///
  /// In en, this message translates to:
  /// **'Noise suppression on'**
  String get devicesNoiseOn;

  /// Screen-reader label of the noise suppression switch while it is off.
  ///
  /// In en, this message translates to:
  /// **'Noise suppression off'**
  String get devicesNoiseOff;

  /// Label of the switch that lowers steady background noise from the microphone.
  ///
  /// In en, this message translates to:
  /// **'Noise suppression'**
  String get devicesNoise;

  /// Settings › Voice & video: explains noise suppression.
  ///
  /// In en, this message translates to:
  /// **'Lowers steady background noise, like a fan or a hum, in what the others hear.'**
  String get devicesNoiseNote;

  /// Microphone panel: short explanation of noise suppression. Fits a 300 px wide panel.
  ///
  /// In en, this message translates to:
  /// **'Lowers a steady fan or hum.'**
  String get devicesNoiseShort;

  /// Settings › Voice & video, under the camera's choice: label of the switch that draws the person's own camera as in a mirror on their screen.
  ///
  /// In en, this message translates to:
  /// **'Mirror my camera'**
  String get devicesMirror;

  /// Under that switch: it changes only how the person sees themselves; what the others get is never mirrored.
  ///
  /// In en, this message translates to:
  /// **'Only how you see yourself here. Others always see your camera as it is.'**
  String get devicesMirrorNote;

  /// Screen-reader label of the mirror switch while it is on.
  ///
  /// In en, this message translates to:
  /// **'Mirror my camera on'**
  String get devicesMirrorOn;

  /// Screen-reader label of the mirror switch while it is off.
  ///
  /// In en, this message translates to:
  /// **'Mirror my camera off'**
  String get devicesMirrorOff;

  /// Settings › Voice & video: button that opens the chosen microphone and shows how loud it is.
  ///
  /// In en, this message translates to:
  /// **'Test microphone'**
  String get devicesTestMic;

  /// The same button while the test runs: closes the microphone.
  ///
  /// In en, this message translates to:
  /// **'Stop test'**
  String get devicesStopMicTest;

  /// Screen-reader label of the bar that shows how loud the microphone is during the test.
  ///
  /// In en, this message translates to:
  /// **'Microphone level'**
  String get devicesMicLevel;

  /// Label of the switch that plays the microphone test through the chosen speaker. Off by default.
  ///
  /// In en, this message translates to:
  /// **'Hear myself'**
  String get devicesHearMyself;

  /// Under that switch: why headphones are better (the speaker's sound can reach the microphone again).
  ///
  /// In en, this message translates to:
  /// **'Plays your microphone through the speaker. Use headphones, or the microphone may pick up the speaker.'**
  String get devicesHearMyselfNote;

  /// Screen-reader label of that switch while it is on.
  ///
  /// In en, this message translates to:
  /// **'Hear myself on'**
  String get devicesHearMyselfOn;

  /// Screen-reader label of that switch while it is off.
  ///
  /// In en, this message translates to:
  /// **'Hear myself off'**
  String get devicesHearMyselfOff;

  /// Under the microphone test: nothing of it is sent to anyone.
  ///
  /// In en, this message translates to:
  /// **'Nothing from the test is sent to anyone.'**
  String get devicesMicTestOnlyYou;

  /// Under the microphone test while in a call: the call is using the microphone, and the test waits.
  ///
  /// In en, this message translates to:
  /// **'The microphone is in use in your call.'**
  String get devicesMicInCall;

  /// Under the microphone test when the computer lists no microphone.
  ///
  /// In en, this message translates to:
  /// **'No microphone is connected.'**
  String get devicesNoMic;

  /// In device lists: the device the system uses by default (first entry).
  ///
  /// In en, this message translates to:
  /// **'System default'**
  String get devicesSystemDefault;

  /// Settings › Voice & video: the chosen device is unplugged, so the system default is used.
  ///
  /// In en, this message translates to:
  /// **'Not connected: the default is used'**
  String get devicesNotConnected;

  /// Disabled entry in the camera list when there is no camera.
  ///
  /// In en, this message translates to:
  /// **'No camera is connected'**
  String get devicesNoCameraShort;

  /// Tooltip of the small arrow beside the microphone button: opens the microphone panel.
  ///
  /// In en, this message translates to:
  /// **'Microphone options'**
  String get devicesMicOptions;

  /// Tooltip of the small arrow beside the deafen button: opens the speaker (sound output) panel.
  ///
  /// In en, this message translates to:
  /// **'Speaker options'**
  String get devicesSpeakerOptions;

  /// Tooltip of the small arrow beside the camera button: opens the camera panel.
  ///
  /// In en, this message translates to:
  /// **'Camera options'**
  String get devicesCameraOptions;

  /// Heading of the microphone panel: the list of microphones.
  ///
  /// In en, this message translates to:
  /// **'Input device'**
  String get devicesInputDevice;

  /// Heading of the speaker panel: the list of sound outputs.
  ///
  /// In en, this message translates to:
  /// **'Output device'**
  String get devicesOutputDevice;

  /// Device panel: the device chosen before is unplugged, so the system default is used.
  ///
  /// In en, this message translates to:
  /// **'The one chosen is not connected: the default is used.'**
  String get devicesChosenMissing;

  /// Last row of a device panel: opens Settings › Voice & video.
  ///
  /// In en, this message translates to:
  /// **'Voice & video settings'**
  String get devicesVoiceSettings;

  /// Name of the file type filter in the system's file dialog when picking a picture.
  ///
  /// In en, this message translates to:
  /// **'Images'**
  String get photoImages;

  /// Error when picking a picture with too many pixels. {n} is the most allowed, in millions of pixels.
  ///
  /// In en, this message translates to:
  /// **'That picture is too large: at most {n} megapixels.'**
  String photoTooLarge(int n);

  /// Error when a picked file is not a picture this app can read.
  ///
  /// In en, this message translates to:
  /// **'That picture could not be read.'**
  String get photoUnreadable;

  /// Screen sharing quality: the screen's own resolution, not scaled down. Short: fits a small pill.
  ///
  /// In en, this message translates to:
  /// **'Source'**
  String get callSource;

  /// Screen sharing quality in a small pill: {resolution} is like "720p", "1080p" or "Source"; {fps} frames per second.
  ///
  /// In en, this message translates to:
  /// **'{resolution} · {fps} fps'**
  String callQuality(String resolution, int fps);

  /// Error toast: the call could not start on this computer.
  ///
  /// In en, this message translates to:
  /// **'Could not set the call up.'**
  String get callSetupFailed;

  /// Error toast (Linux): no sound server (PulseAudio or PipeWire) answers, so the call is not started.
  ///
  /// In en, this message translates to:
  /// **'Could not reach this computer\'s sound system, so the call cannot start.'**
  String get callNoSoundSystem;

  /// Toast in a call: the microphone chosen went away (a headset unplugged); {name} is the input used now, as the system names it, or "System default".
  ///
  /// In en, this message translates to:
  /// **'Your microphone was disconnected: using {name} now.'**
  String callMicGone(String name);

  /// Toast in a call: the microphone chosen came back and is used again.
  ///
  /// In en, this message translates to:
  /// **'Your microphone is back.'**
  String get callMicBack;

  /// Error toast in a call: the microphone is on but nothing was read from it for a while (refused by the system, or not connected). On macOS ({system} = macos) and Windows ({system} = windows) it also says where it is allowed; keep the settings' names as each system has them in the language.
  ///
  /// In en, this message translates to:
  /// **'{system, select, macos{Could not use the microphone. Check that it is connected and allowed in System Settings › Privacy & Security.} windows{Could not use the microphone. Check that it is connected and allowed in Settings › Privacy & security › Microphone.} other{Could not use the microphone. Check that it is connected.}}'**
  String callMicSilent(String system);

  /// Error toast in a call: the microphone is on but the computer lists no sound input at all.
  ///
  /// In en, this message translates to:
  /// **'No microphone is connected.'**
  String get callNoMicrophone;

  /// Error toast: the microphone could not be opened, or reads nothing. On macOS ({system} = macos) and Windows ({system} = windows) it says where to allow it; keep "System Settings › Privacy & Security" (macOS) and "Settings › Privacy & security › Microphone" (Windows) as each system names them in the language.
  ///
  /// In en, this message translates to:
  /// **'{system, select, macos{Could not use the microphone. Allow it in System Settings › Privacy & Security.} windows{Could not use the microphone. Allow it in Settings › Privacy & security › Microphone.} other{Could not use the microphone.}}'**
  String callMicFailed(String system);

  /// Error toast: the screen could not be shared. On macOS ({system} = macos) it says where to allow it (screen recording); keep "System Settings › Privacy & Security" as macOS names it in the language.
  ///
  /// In en, this message translates to:
  /// **'{system, select, macos{Could not share the screen. Allow it in System Settings › Privacy & Security.} other{Could not share the screen.}}'**
  String callScreenFailed(String system);

  /// Error toast: the screen or window being shared could no longer be captured when it was captured again at another quality (a shared window that closed, for example), so sharing stopped.
  ///
  /// In en, this message translates to:
  /// **'Screen sharing stopped: what you were sharing can no longer be captured.'**
  String get callShareLost;

  /// Error toast in a call: the connection with a participant was closed because their copy sent video far larger than any copy of the app sends. {name} is their name.
  ///
  /// In en, this message translates to:
  /// **'Lost the connection with {name}.'**
  String callKeptOut(String name);

  /// A call connection's level, in a word (shown on a small pill and after "Your connection:").
  ///
  /// In en, this message translates to:
  /// **'Good'**
  String get qualityGood;

  /// A call connection's level, in a word: usable but not good.
  ///
  /// In en, this message translates to:
  /// **'Okay'**
  String get qualityOkay;

  /// A call connection's level, in a word: bad, people may sound choppy. Also shown alone on a narrow card.
  ///
  /// In en, this message translates to:
  /// **'Poor'**
  String get qualityPoor;

  /// A call connection's level, in a word: nothing arrives.
  ///
  /// In en, this message translates to:
  /// **'Lost'**
  String get qualityLost;

  /// A network delay in milliseconds.
  ///
  /// In en, this message translates to:
  /// **'{ms} ms'**
  String qualityDelay(int ms);

  /// A share of lost packets in percent; {value} is already formatted, like 0.1 or 12. Use the language's own spacing before %.
  ///
  /// In en, this message translates to:
  /// **'{value} %'**
  String qualityPercent(String value);

  /// Stands for a group member whose name is not known, who carries the connection: "it goes through another member". Lower case.
  ///
  /// In en, this message translates to:
  /// **'another member'**
  String get qualityAnotherMember;

  /// Call connection details: nothing arrives from a person. {name} is the person, {seconds} how many seconds.
  ///
  /// In en, this message translates to:
  /// **'Nothing has come from {name} for {seconds} s. Fogata keeps looking for them; they come back on their own as soon as their network does.'**
  String qualityLostHint(String name, int seconds);

  /// Call connection details: both people's networks are bad. {name} is the other person.
  ///
  /// In en, this message translates to:
  /// **'Both networks are poor right now: you and {name} may sound choppy to each other.'**
  String qualityBothPoor(String name);

  /// Call connection details: the other person's own network is bad. {name} is that person.
  ///
  /// In en, this message translates to:
  /// **'The trouble is on {name}’s side: they sound choppy to everyone. Their Wi-Fi or mobile data is poor right now.'**
  String qualityTheirSide(String name);

  /// Call connection details: this person's own network is bad. {name} is the other person.
  ///
  /// In en, this message translates to:
  /// **'The trouble is on your side: you may sound choppy to {name} and the others. Your Wi-Fi or mobile data is poor right now.'**
  String qualityYourSideWith(String name);

  /// Call connection details: the other person's network is somewhat slow. {name} is that person.
  ///
  /// In en, this message translates to:
  /// **'{name}’s network is a bit slow right now: they may sound choppy to everyone now and then.'**
  String qualityTheirsSlow(String name);

  /// Call connection details: all is fine with the other person. {name} is that person.
  ///
  /// In en, this message translates to:
  /// **'{name} hears you and you hear them without trouble.'**
  String qualityAllGoodWith(String name);

  /// Call connection details: this person's network is somewhat slow. {name} is the other person.
  ///
  /// In en, this message translates to:
  /// **'Your network is a bit slow right now: you may sound choppy to {name} now and then.'**
  String qualityMineSlowWith(String name);

  /// Call connection details: the connection goes through another group member and is slower. {name} is the other person, {carrier} the member in between (or "another member").
  ///
  /// In en, this message translates to:
  /// **'{name}’s own network is fine; the way between you two is slower because it goes through {carrier}. They may sound choppy to you now and then; the others hear them well.'**
  String qualityWayThrough(String name, String carrier);

  /// Call connection details: the connection between the two is slow though the other's network is fine. {name} is the other person.
  ///
  /// In en, this message translates to:
  /// **'{name}’s own network is fine; the way between you two is slow right now. They may sound choppy to you now and then; the others hear them well.'**
  String qualityWaySlowFine(String name);

  /// Call connection details: the connection between the two is slow (level = okay) or poor (other); it may be either network. {name} is the other person.
  ///
  /// In en, this message translates to:
  /// **'{level, select, okay{The way between you and {name} is a bit slow right now; it may be either network. You may sound choppy to each other now and then.} other{The way between you and {name} is poor right now; it may be either network. You may sound choppy to each other.}}'**
  String qualityWay(String level, String name);

  /// Call connection details: like the sentence without a member, adding that the connection goes through {carrier}, another group member (or "another member"). level = okay for a bit slow, other for poor.
  ///
  /// In en, this message translates to:
  /// **'{level, select, okay{The way between you and {name} is a bit slow right now; it may be either network. You may sound choppy to each other now and then. It goes through {carrier}.} other{The way between you and {name} is poor right now; it may be either network. You may sound choppy to each other. It goes through {carrier}.}}'**
  String qualityWayVia(String level, String name, String carrier);

  /// Call connection details: this person's own network is bad.
  ///
  /// In en, this message translates to:
  /// **'The trouble is on your side: you may sound choppy to the others. Your Wi-Fi or mobile data is poor right now.'**
  String get qualityYourSide;

  /// Call connection details: this person's own network is somewhat slow.
  ///
  /// In en, this message translates to:
  /// **'Your network is a bit slow right now: you may sound choppy to the others now and then.'**
  String get qualityMineSlow;

  /// Call connection details: nothing arrives from the only other person. {name} is that person, {seconds} how many seconds.
  ///
  /// In en, this message translates to:
  /// **'Nothing has come from {name} for {seconds} s. Fogata keeps looking for them.'**
  String qualityLostShort(String name, int seconds);

  /// Call connection details: all is fine with the only other person. {name} is that person.
  ///
  /// In en, this message translates to:
  /// **'All is well: you and {name} hear each other without trouble.'**
  String qualityAllWellWith(String name);

  /// Call connection details: one of several notes, joined into "All is well on your side. …". Starts in lower case (the first note gets a capital letter); no final full stop. {name} is a person.
  ///
  /// In en, this message translates to:
  /// **'{name} dropped and Fogata is looking for them again'**
  String qualityNoteDropped(String name);

  /// Call connection details: one of several notes, joined into "All is well on your side. …". Starts in lower case (the first note gets a capital letter); no final full stop. {name} is a person.
  ///
  /// In en, this message translates to:
  /// **'{name}’s network is poor, so they sound choppy to everyone'**
  String qualityNoteTheirsPoor(String name);

  /// Call connection details: one of several notes, joined into "All is well on your side. …". Starts in lower case (the first note gets a capital letter); no final full stop. {name} is a person.
  ///
  /// In en, this message translates to:
  /// **'{name}’s network is a bit slow'**
  String qualityNoteTheirsSlow(String name);

  /// Call connection details: one of several notes, joined into "All is well on your side. …". Starts in lower case (the first note gets a capital letter); no final full stop. {name} is a person, {carrier} the group member in between (or "another member").
  ///
  /// In en, this message translates to:
  /// **'{name} comes to you through {carrier}'**
  String qualityNoteThrough(String name, String carrier);

  /// Call connection details: one of several notes, joined into "All is well on your side. …". Starts in lower case (the first note gets a capital letter); no final full stop. {name} is a person; level = okay for a bit slow, other for poor.
  ///
  /// In en, this message translates to:
  /// **'{level, select, okay{the way to {name} is a bit slow right now} other{the way to {name} is poor right now}}'**
  String qualityNoteWay(String level, String name);

  /// What joins the notes in "All is well on your side. Leo’s network is a bit slow; the way to Ana is poor right now." Keep the space if the language uses one.
  ///
  /// In en, this message translates to:
  /// **'; '**
  String get qualityNoteSeparator;

  /// Call connection details: everything is fine.
  ///
  /// In en, this message translates to:
  /// **'All is well: you hear everyone and everyone hears you.'**
  String get qualityAllWell;

  /// Call connection details: this person's network is fine; {notes} are notes about others, joined, the first one capitalized.
  ///
  /// In en, this message translates to:
  /// **'All is well on your side. {notes}.'**
  String qualityAllWellMine(String notes);

  /// Screen-reader label of the connection bars for this person: {level} is a word like Good or Poor; activating it shows the details.
  ///
  /// In en, this message translates to:
  /// **'Your connection: {level}. Show details'**
  String qualityYourDetails(String level);

  /// Screen-reader label of a person's connection bars: {name} is the person, {level} a word like Good or Poor; activating it shows the details.
  ///
  /// In en, this message translates to:
  /// **'{name}’s connection: {level}. Show details'**
  String qualityTheirDetails(String name, String level);

  /// Shown on a person's tile and connection pill while nothing arrives from them and Fogata reconnects.
  ///
  /// In en, this message translates to:
  /// **'Reconnecting…'**
  String get qualityReconnecting;

  /// Screen-reader label of the spinner beside a person who is reconnecting.
  ///
  /// In en, this message translates to:
  /// **'Reconnecting'**
  String get qualityReconnectingLabel;

  /// Screen-reader label of small connection bars beside a person. level: good, okay, weak (poor) or lost.
  ///
  /// In en, this message translates to:
  /// **'{level, select, good{Good connection} okay{Okay connection} weak{Poor connection} other{Lost connection}}'**
  String qualityLevelConnection(String level);

  /// On this person's card at the bottom left while their connection is poor; replaced by the word "Poor" alone when it does not fit.
  ///
  /// In en, this message translates to:
  /// **'Poor connection'**
  String get qualityPoorConnection;

  /// One-line note in a call while this person's network is poor. Cut with "…" when too long.
  ///
  /// In en, this message translates to:
  /// **'Your network is poor: you may sound choppy to others'**
  String get qualityMinePoorBanner;

  /// Connection details: row label for this person's own network.
  ///
  /// In en, this message translates to:
  /// **'Your network'**
  String get qualityYourNetwork;

  /// Connection details: row label for the connection to one person. {name} is that person.
  ///
  /// In en, this message translates to:
  /// **'To {name}'**
  String qualityTo(String name);

  /// Connection details: row label for the connection between this person and {name}.
  ///
  /// In en, this message translates to:
  /// **'Between you and {name}'**
  String qualityBetween(String name);

  /// Connection details: row label for the other person's own network. {name} is that person.
  ///
  /// In en, this message translates to:
  /// **'{name}’s own network'**
  String qualityTheirNetwork(String name);

  /// Connection details: label of when something last arrived from the person.
  ///
  /// In en, this message translates to:
  /// **'Last heard'**
  String get qualityLastHeard;

  /// Connection details: how many seconds ago something last arrived.
  ///
  /// In en, this message translates to:
  /// **'{n} s ago'**
  String qualitySecondsAgo(int n);

  /// Connection details: label of how the connection goes (directly, through a router, through a member).
  ///
  /// In en, this message translates to:
  /// **'Path'**
  String get qualityPath;

  /// Connection details: the path while Fogata looks for a person who dropped.
  ///
  /// In en, this message translates to:
  /// **'Looking for them again'**
  String get qualityLookingAgain;

  /// Connection details: label of the network delay (round trip time).
  ///
  /// In en, this message translates to:
  /// **'Delay'**
  String get qualityDelayLabel;

  /// Connection details: label of the share of network packets lost.
  ///
  /// In en, this message translates to:
  /// **'Lost packets'**
  String get qualityLostPackets;

  /// Title of the connection details for this person.
  ///
  /// In en, this message translates to:
  /// **'Your connection'**
  String get qualityYourConnection;

  /// Title of the connection details for another person. {name} is that person.
  ///
  /// In en, this message translates to:
  /// **'{name}’s connection'**
  String qualityTheirConnection(String name);

  /// Screen-reader label of a voice room's people count when the room is full.
  ///
  /// In en, this message translates to:
  /// **'Full, {count} of {max} people'**
  String roomCountFullLabel(int count, int max);

  /// Screen-reader label of a voice room's people count: {count} people inside, {max} at most.
  ///
  /// In en, this message translates to:
  /// **'{count} of {max} people'**
  String roomCountLabel(int count, int max);

  /// Short word in a small pill, before a count like "8/8", when a voice room is full. Keep the " · " separator and the trailing space.
  ///
  /// In en, this message translates to:
  /// **'Full · '**
  String get roomCountFullPrefix;

  /// Short word before a count like "8/8" when a voice room is full. Keep the trailing space.
  ///
  /// In en, this message translates to:
  /// **'Full '**
  String get roomCountFullWord;

  /// A person's voice menu (right click on someone in a call): title of the slider for how loud their voice plays on this computer; also a screen-reader action that opens the menu.
  ///
  /// In en, this message translates to:
  /// **'Voice volume'**
  String get volumeVoice;

  /// A person's voice menu: small note under their name; the volume changes affect only this computer.
  ///
  /// In en, this message translates to:
  /// **'Only you hear these changes'**
  String get volumeOnlyYouHear;

  /// A person's voice menu: title of the slider for how loud the sound of their shared screen plays.
  ///
  /// In en, this message translates to:
  /// **'Screen sound'**
  String get volumeScreenSound;

  /// A volume in percent (0 to 200), like "100%". Use the language's usual way to write percentages.
  ///
  /// In en, this message translates to:
  /// **'{n}%'**
  String volumePercent(int n);

  /// A person's voice menu (when they share a screen): switch that stops playing their voice on this computer only.
  ///
  /// In en, this message translates to:
  /// **'Mute voice for me'**
  String get volumeMuteVoiceForMe;

  /// A person's voice menu: switch that stops playing their voice on this computer only.
  ///
  /// In en, this message translates to:
  /// **'Mute for me'**
  String get volumeMuteForMe;

  /// A person's voice menu: switch that stops playing the sound of their shared screen on this computer only.
  ///
  /// In en, this message translates to:
  /// **'Mute screen sound for me'**
  String get volumeMuteScreenForMe;

  /// A person's voice menu: menu item that sets the volume back to normal.
  ///
  /// In en, this message translates to:
  /// **'Back to 100%'**
  String get volumeBackTo100;

  /// A person's voice menu: small heading in capitals over the actions a group manager can take on someone ranked below them.
  ///
  /// In en, this message translates to:
  /// **'BELOW YOU'**
  String get volumeBelowYou;

  /// A person's voice menu: menu item that takes the person out of the voice room {room} (the room's name).
  ///
  /// In en, this message translates to:
  /// **'Remove from {room}'**
  String volumeRemoveFrom(String room);

  /// Title of the dialog that asks before taking the person {name} out of the voice room {room}.
  ///
  /// In en, this message translates to:
  /// **'Remove {name} from {room}?'**
  String volumeRemoveTitle(String name, String room);

  /// Text of the dialog that asks before taking the person {name} out of the voice room {room}.
  ///
  /// In en, this message translates to:
  /// **'{name} leaves {room} now and can come back in at once. A time out keeps someone out for a while.'**
  String volumeRemoveText(String name, String room);

  /// A person's voice menu: menu item that opens how long to keep the person out of the voice room {room}.
  ///
  /// In en, this message translates to:
  /// **'Time out of {room}'**
  String volumeTimeOutOf(String room);

  /// Time-out menu: heading over the durations (5 minutes, 1 hour…) the person {name} is kept out of the voice room {room}.
  ///
  /// In en, this message translates to:
  /// **'{name} leaves {room} and cannot come back for:'**
  String volumeTimeOutText(String name, String room);

  /// Time-out menu: last choice, keeps the person out with no end until a higher-ranked member lets them back.
  ///
  /// In en, this message translates to:
  /// **'Until someone above them lets them back'**
  String get volumeUntilLetBack;

  /// Label over a picture of the screen this computer shares (in a call, or in the small floating view).
  ///
  /// In en, this message translates to:
  /// **'Your screen'**
  String get pipYourScreen;

  /// Label over a picture of the screen someone shares; {name} is the person's name.
  ///
  /// In en, this message translates to:
  /// **'{name}’s screen'**
  String pipScreenOf(String name);

  /// Label over the picture of this computer's own camera.
  ///
  /// In en, this message translates to:
  /// **'Your camera'**
  String get pipYourCamera;

  /// Screen-reader label of the small floating video in a corner: clicking it goes back to the direct call.
  ///
  /// In en, this message translates to:
  /// **'Back to the call'**
  String get pipBackToCall;

  /// Screen-reader label of the small floating video in a corner: clicking it goes back to the voice room.
  ///
  /// In en, this message translates to:
  /// **'Back to the room'**
  String get pipBackToRoom;

  /// Tooltip of the button on the small floating view of someone's shared screen: stop watching it.
  ///
  /// In en, this message translates to:
  /// **'Stop watching'**
  String get pipStopWatching;

  /// Title of the panel that adds friends to a direct call that is under way.
  ///
  /// In en, this message translates to:
  /// **'Add to the call'**
  String get addToCallTitle;

  /// Add to the call panel: note under the title; {n} is the most people a call takes.
  ///
  /// In en, this message translates to:
  /// **'Friends who are online. Up to {n} people in a call.'**
  String addToCallNote(int n);

  /// Add to the call panel: placeholder of the search field.
  ///
  /// In en, this message translates to:
  /// **'Search friends'**
  String get addToCallSearch;

  /// Add to the call panel: empty list, this person has no friends yet.
  ///
  /// In en, this message translates to:
  /// **'No friends yet.'**
  String get addToCallNoFriends;

  /// Add to the call panel: no friend's name matches what was typed.
  ///
  /// In en, this message translates to:
  /// **'No friend by that name.'**
  String get addToCallNoMatch;

  /// Add to the call panel: small label beside a friend already in the call.
  ///
  /// In en, this message translates to:
  /// **'In the call'**
  String get addToCallInCall;

  /// Add to the call panel: small label beside a friend being rung now.
  ///
  /// In en, this message translates to:
  /// **'Calling…'**
  String get addToCallCalling;

  /// Add to the call panel: small label beside a friend who is offline.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get addToCallOffline;

  /// Add to the call panel: small button that rings this friend into the call.
  ///
  /// In en, this message translates to:
  /// **'Call'**
  String get addToCallCall;

  /// Title of the panel that sets how this computer's shared screen is sent (resolution and frame rate).
  ///
  /// In en, this message translates to:
  /// **'Stream quality'**
  String get streamQualityTitle;

  /// Stream quality panel: note under the title while the screen is being shared.
  ///
  /// In en, this message translates to:
  /// **'How your screen reaches the others. A change applies at once.'**
  String get streamQualityNoteSharing;

  /// Stream quality panel: note under the title while the screen is not being shared.
  ///
  /// In en, this message translates to:
  /// **'How your screen reaches the others when you share it.'**
  String get streamQualityNote;

  /// Stream quality panel: label of the resolution choice (720p, 1080p, Source).
  ///
  /// In en, this message translates to:
  /// **'Resolution'**
  String get streamQualityResolution;

  /// Stream quality panel: option that sends the screen at its own size; {lines} is its height in pixels (like 1440).
  ///
  /// In en, this message translates to:
  /// **'Source ({lines}p)'**
  String streamQualitySourceLines(int lines);

  /// Stream quality panel: option that sends the screen at its own size. Short: fits a small segment.
  ///
  /// In en, this message translates to:
  /// **'Source'**
  String get streamQualitySource;

  /// Stream quality panel: label of the frames-per-second choice.
  ///
  /// In en, this message translates to:
  /// **'Frame rate'**
  String get streamQualityFrameRate;

  /// A frame rate, frames per second (like "30 fps"). Short: fits a small segment.
  ///
  /// In en, this message translates to:
  /// **'{n} fps'**
  String streamQualityFps(int n);

  /// Stream quality panel: note that higher quality uses more upload bandwidth and computer power.
  ///
  /// In en, this message translates to:
  /// **'Higher settings take more of your upload and your computer.'**
  String get streamQualityHigherNote;

  /// Stream quality panel: stands for a watcher whose name is not known.
  ///
  /// In en, this message translates to:
  /// **'Someone'**
  String get streamQualitySomeone;

  /// Stream quality panel: why one watcher {name} gets a smaller picture than chosen. {others} is "yes" when other people watch too, which adds the last sentence (keep its leading space if the language separates sentences with one).
  ///
  /// In en, this message translates to:
  /// **'{name}’s connection can’t take more right now, so they get a smaller picture.{others, select, yes{ The others get what you chose.} other{}}'**
  String streamQualityLoweredOne(String name, String others);

  /// Stream quality panel: why several watchers get a smaller picture than chosen; {names} is their names separated by commas. {others} is "yes" when other people watch too, which adds the last sentence.
  ///
  /// In en, this message translates to:
  /// **'Some connections can’t take more right now: {names} get a smaller picture.{others, select, yes{ The others get what you chose.} other{}}'**
  String streamQualityLoweredMany(String names, String others);

  /// Stream quality panel: small heading in capitals over what each watcher receives now.
  ///
  /// In en, this message translates to:
  /// **'GOING OUT NOW'**
  String get streamQualityGoingOut;

  /// Stream quality panel: nobody watches the shared screen yet.
  ///
  /// In en, this message translates to:
  /// **'Nobody is watching yet.'**
  String get streamQualityNobody;

  /// Stream quality panel: what one watcher receives now, like "720p · 30 fps".
  ///
  /// In en, this message translates to:
  /// **'{height}p · {fps} fps'**
  String streamQualityNow(int height, int fps);

  /// Incoming direct call card: title; {name} is the friend ringing.
  ///
  /// In en, this message translates to:
  /// **'{name} is calling'**
  String callLayersIsCalling(String name);

  /// What a screen reader adds after saying someone is calling, while the focus is elsewhere (in a message being written): the F6 key moves the focus to the call's card, to answer or decline it. Not shown on screen.
  ///
  /// In en, this message translates to:
  /// **'F6 goes to the call'**
  String get callLayersGoToCall;

  /// Incoming call card: who else is in the call; {names} is a list of names ("Marta and Leo").
  ///
  /// In en, this message translates to:
  /// **'With {names}'**
  String callLayersWith(String names);

  /// Incoming call card: label before the caller's short fingerprint code (like "C07N·50S4"). Keep " · " and the trailing space.
  ///
  /// In en, this message translates to:
  /// **'Direct call · '**
  String get callLayersDirectCall;

  /// Incoming call card: switch to answer the call muted.
  ///
  /// In en, this message translates to:
  /// **'Answer with the microphone off'**
  String get callLayersAnswerMicOff;

  /// Small incoming call card in the corner while in a voice room: warns that answering leaves the room {room} (its name).
  ///
  /// In en, this message translates to:
  /// **'Answering leaves {room}'**
  String callLayersAnsweringLeaves(String room);

  /// Small incoming call card: button that leaves the current voice room and answers. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Leave and answer'**
  String get callLayersLeaveAndAnswer;

  /// Tooltip of a disabled Call button: this person and the friend are both in the voice room {room} already.
  ///
  /// In en, this message translates to:
  /// **'You’re both in {room}.'**
  String callLayersBothIn(String room);

  /// Tooltip of the close button on the missed-call note: hide the note.
  ///
  /// In en, this message translates to:
  /// **'Dismiss'**
  String get callLayersDismiss;

  /// Card in the corner for the call under way while another page shows: its title; {names} is who is in the call ("Marta and Leo").
  ///
  /// In en, this message translates to:
  /// **'Call with {names}'**
  String callLayersCallWith(String names);

  /// Card in the corner for the voice room this computer is in, when the group has no name to show.
  ///
  /// In en, this message translates to:
  /// **'Room'**
  String get callLayersRoom;

  /// Screen-reader label of the card in the corner: go back to the call or room {title}.
  ///
  /// In en, this message translates to:
  /// **'Back to {title}'**
  String callLayersBackTo(String title);

  /// Card in the corner: the call rings, and for how long ({time} like "0:42").
  ///
  /// In en, this message translates to:
  /// **'Ringing · {time}'**
  String callLayersRinging(String time);

  /// Card in the corner: in a direct call, and for how long ({time} like "12:05").
  ///
  /// In en, this message translates to:
  /// **'In call · {time}'**
  String callLayersInCall(String time);

  /// Card in the corner: alone in the voice room, and for how long ({time} like "12:05").
  ///
  /// In en, this message translates to:
  /// **'Only you · {time}'**
  String callLayersOnlyYou(String time);

  /// Card in the corner: how many people are in the voice room (always 2 or more) and for how long ({time} like "12:05").
  ///
  /// In en, this message translates to:
  /// **'{n, plural, other{{n} in the room}} · {time}'**
  String callLayersInRoom(int n, String time);

  /// Tooltip of the microphone button on the call card in the corner: turn the microphone off.
  ///
  /// In en, this message translates to:
  /// **'Mute'**
  String get callLayersMute;

  /// Tooltip of the microphone button on the call card in the corner: turn the microphone on.
  ///
  /// In en, this message translates to:
  /// **'Unmute'**
  String get callLayersUnmute;

  /// Tooltip of the red button on the call card in the corner: end the direct call.
  ///
  /// In en, this message translates to:
  /// **'Hang up'**
  String get callLayersHangUp;

  /// Tooltip of the red button on the room card in the corner: leave the voice room.
  ///
  /// In en, this message translates to:
  /// **'Leave the room'**
  String get callLayersLeaveRoom;

  /// Dialog title, asked before calling a friend while another friend {name} is ringing this computer.
  ///
  /// In en, this message translates to:
  /// **'Decline {name}’s call?'**
  String roomDeclineCallTitle(String name);

  /// Dialog text, asked before calling the friend {callee} while the friend {caller} is ringing this computer.
  ///
  /// In en, this message translates to:
  /// **'{caller} is calling you. You can be in one call at a time: calling {callee} declines it.'**
  String roomDeclineCallText(String caller, String callee);

  /// Dialog text, asked before calling someone while the friend {caller} is ringing this computer.
  ///
  /// In en, this message translates to:
  /// **'{caller} is calling you. You can be in one call at a time: calling someone else declines it.'**
  String roomDeclineCallTextSomeone(String caller);

  /// Dialog button: decline the ringing call and make the new one.
  ///
  /// In en, this message translates to:
  /// **'Decline and call'**
  String get roomDeclineAndCall;

  /// Dialog title, asked before entering a room or another call while in a direct call with {name}.
  ///
  /// In en, this message translates to:
  /// **'Hang up the call with {name}?'**
  String roomHangUpTitle(String name);

  /// Dialog text, asked before entering a room or another call while in a direct call.
  ///
  /// In en, this message translates to:
  /// **'You can be in one call or room at a time: going on ends the call.'**
  String get roomHangUpText;

  /// Button: end the direct call.
  ///
  /// In en, this message translates to:
  /// **'Hang up'**
  String get roomHangUp;

  /// Dialog title, asked when the last person in a voice room leaves it (the room then closes).
  ///
  /// In en, this message translates to:
  /// **'Leave and close the room?'**
  String get roomCloseTitle;

  /// Dialog text, asked when the last person in a voice room leaves it.
  ///
  /// In en, this message translates to:
  /// **'You are the last one here. When you leave, the room closes and what was said is gone from this computer; what others downloaded stays on theirs.'**
  String get roomCloseText;

  /// Leave and close dialog: row label, followed by how many chat messages the room holds.
  ///
  /// In en, this message translates to:
  /// **'Chat messages'**
  String get roomCloseChatMessages;

  /// Leave and close dialog: row label, followed by how many files this person shared in the room.
  ///
  /// In en, this message translates to:
  /// **'Files you shared'**
  String get roomCloseFilesShared;

  /// Leave and close dialog: button that stays in the room.
  ///
  /// In en, this message translates to:
  /// **'Stay'**
  String get roomStay;

  /// Button: leave the voice room as its last person, which closes it.
  ///
  /// In en, this message translates to:
  /// **'Leave and close'**
  String get roomLeaveAndClose;

  /// Pill beside a room's name: the room is live, followed by how long it has been open (like "12:05"). Keep " · " and the trailing space.
  ///
  /// In en, this message translates to:
  /// **'Live · '**
  String get roomLivePrefix;

  /// Pill beside a room's name: this computer is connecting to the room. Short.
  ///
  /// In en, this message translates to:
  /// **'Connecting'**
  String get roomPillConnecting;

  /// Pill beside a room's name: nobody else is in the room. Short.
  ///
  /// In en, this message translates to:
  /// **'Only you'**
  String get roomPillOnlyYou;

  /// Pill beside a room's name: connecting to the room failed. Short.
  ///
  /// In en, this message translates to:
  /// **'Error'**
  String get roomPillError;

  /// Warning bar in a voice room (bold first sentence) when everyone else left.
  ///
  /// In en, this message translates to:
  /// **'You are the last one in the room.'**
  String get roomLastOne;

  /// Warning bar in a voice room (bold first sentence) when nobody else came in.
  ///
  /// In en, this message translates to:
  /// **'Only you are in the room.'**
  String get roomOnlyYouIn;

  /// Warning bar in a voice room: second sentence, after "Only you are in the room." Starts with a space when the language separates sentences with one.
  ///
  /// In en, this message translates to:
  /// **' When you leave, it closes and everything in it is gone.'**
  String get roomAloneWarning;

  /// Tooltip of the arrow that scrolls the strip of people under a large picture back to the left.
  ///
  /// In en, this message translates to:
  /// **'Earlier people'**
  String get roomEarlierPeople;

  /// Tooltip of the arrow that scrolls the strip of people under a large picture to the right.
  ///
  /// In en, this message translates to:
  /// **'More people'**
  String get roomMorePeople;

  /// Name on this computer's own tile in a call; {name} is this person's name.
  ///
  /// In en, this message translates to:
  /// **'{name} (you)'**
  String roomTileYou(String name);

  /// Name on a person's tile when they are in the room without sound: their computer could not start the call (no sound system).
  ///
  /// In en, this message translates to:
  /// **'{name} · no sound'**
  String roomTileNoSound(String name);

  /// Name on a person's tile while their computer is still being reached.
  ///
  /// In en, this message translates to:
  /// **'{name} · connecting'**
  String roomTileConnecting(String name);

  /// Label on the tile of the screen this computer shares, with how many people watch it (1 or more).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, other{Your screen · {n} watching}}'**
  String roomYourScreenWatching(int n);

  /// Name on a person's tile while nothing is heard from them: their connection dropped for a moment. Not the away status (nobody using the computer): a different word.
  ///
  /// In en, this message translates to:
  /// **'{name} · no connection'**
  String roomTileAway(String name);

  /// What a screen reader says for a person's tile in a room while their microphone is off. Not shown on screen.
  ///
  /// In en, this message translates to:
  /// **'{name}, microphone off'**
  String roomTileMicOff(String name);

  /// What a screen reader adds to a person's tile in a room when this device muted them for itself only (from their volume menu). {said} is the rest of what the tile says (their name). Not shown on screen.
  ///
  /// In en, this message translates to:
  /// **'{said}, muted for you'**
  String roomTileMutedByYou(String said);

  /// What a screen reader adds to a person'\''s tile in a room when their voice plays louder or quieter than usual on this device. {said} is the rest of what the tile says; {percent} is the volume, like 150%. Not shown on screen.
  ///
  /// In en, this message translates to:
  /// **'{said}, volume {percent}'**
  String roomTileVolume(String said, String percent);

  /// Name on the tile of someone who left a direct call a moment ago and may come back.
  ///
  /// In en, this message translates to:
  /// **'{name} left · may come back'**
  String roomTileLeft(String name);

  /// Card for someone's shared screen not watched yet.
  ///
  /// In en, this message translates to:
  /// **'{name} is sharing the screen'**
  String roomIsSharing(String name);

  /// Card for someone's shared screen: nothing of it reaches this computer until Watch is pressed.
  ///
  /// In en, this message translates to:
  /// **'Nothing is sent to you until you watch'**
  String get roomNothingSentUntilWatch;

  /// Button on the card of a shared screen: start watching it. Short.
  ///
  /// In en, this message translates to:
  /// **'Watch'**
  String get roomWatch;

  /// Small red label in capitals on a shared screen that is live. Very short.
  ///
  /// In en, this message translates to:
  /// **'LIVE'**
  String get roomLive;

  /// On the tile of the screen or window this computer shares, in place of its live picture: says that it is being shared. Short.
  ///
  /// In en, this message translates to:
  /// **'You\'re sharing your screen'**
  String get roomSharingYourScreen;

  /// Button on the tile of the screen or window this computer shares: shows its live picture, enlarged. Short.
  ///
  /// In en, this message translates to:
  /// **'Show preview'**
  String get roomShowPreview;

  /// Small note under the Show preview button: showing the live picture of your own share while the window shared is Fogata's makes the picture repeat inside itself, like two facing mirrors.
  ///
  /// In en, this message translates to:
  /// **'Showing it while you share Fogata itself makes an endless mirror'**
  String get roomPreviewMirror;

  /// Tooltip over the eye and count on a shared screen: nobody watches it.
  ///
  /// In en, this message translates to:
  /// **'Nobody watching'**
  String get roomNobodyWatching;

  /// Tooltip over the eye and count on a shared screen: how many people watch it.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 person watching} other{{n} people watching}}'**
  String roomPeopleWatching(int n);

  /// Tooltip of the button that leaves full screen view of a shared screen or camera.
  ///
  /// In en, this message translates to:
  /// **'Exit full screen'**
  String get roomExitFullScreen;

  /// Tooltip of the button that shows a shared screen or camera in full screen.
  ///
  /// In en, this message translates to:
  /// **'Full screen'**
  String get roomFullScreen;

  /// Tooltip of the speaker button on a watched shared screen, while its sound is muted here.
  ///
  /// In en, this message translates to:
  /// **'Play the screen’s sound'**
  String get roomPlayScreenSound;

  /// Tooltip of the speaker button on a watched shared screen: stop playing its sound here.
  ///
  /// In en, this message translates to:
  /// **'Mute the screen’s sound'**
  String get roomMuteScreenSound;

  /// Small pill over someone's shared screen or camera: how their connection is. Very short.
  ///
  /// In en, this message translates to:
  /// **'Poor'**
  String get roomQualityPoor;

  /// Small pill over someone's shared screen or camera: the connection is middling. Very short.
  ///
  /// In en, this message translates to:
  /// **'Okay'**
  String get roomQualityOkay;

  /// Small pill over someone's shared screen or camera: the connection is good. Very short.
  ///
  /// In en, this message translates to:
  /// **'Good'**
  String get roomQualityGood;

  /// Title of the connection details under the quality pill of a shared screen or camera.
  ///
  /// In en, this message translates to:
  /// **'Poor connection'**
  String get roomQualityPoorTitle;

  /// Title of the connection details under the quality pill of a shared screen or camera.
  ///
  /// In en, this message translates to:
  /// **'Okay connection'**
  String get roomQualityOkayTitle;

  /// Title of the connection details under the quality pill of a shared screen or camera.
  ///
  /// In en, this message translates to:
  /// **'Good connection'**
  String get roomQualityGoodTitle;

  /// Small pill over someone's shared screen or camera, before the connection is measured. Very short.
  ///
  /// In en, this message translates to:
  /// **'Measuring'**
  String get roomQualityMeasuring;

  /// Connection details of a shared screen or camera: row label for the network delay.
  ///
  /// In en, this message translates to:
  /// **'Delay'**
  String get roomQualityDelay;

  /// A delay in milliseconds; {n} is already formatted.
  ///
  /// In en, this message translates to:
  /// **'{n} ms'**
  String roomQualityMs(String n);

  /// Connection details of a shared screen or camera: row label for the share of network packets lost.
  ///
  /// In en, this message translates to:
  /// **'Lost packets'**
  String get roomQualityLost;

  /// A share in percent; {n} is already formatted (like 1.5). Use the language's usual way to write percentages.
  ///
  /// In en, this message translates to:
  /// **'{n} %'**
  String roomQualityPercent(String n);

  /// Connection details of a shared screen or camera: row label for the picture size and frame rate.
  ///
  /// In en, this message translates to:
  /// **'Picture'**
  String get roomQualityPicture;

  /// Connection details of a shared screen or camera: row label for how the connection goes (local network, direct, through a member).
  ///
  /// In en, this message translates to:
  /// **'Path'**
  String get roomQualityPath;

  /// Connection details of a shared screen or camera, when the connection is poor.
  ///
  /// In en, this message translates to:
  /// **'The picture may freeze. Fewer people sharing at once helps.'**
  String get roomQualityMayFreeze;

  /// Connection details of a shared screen or camera, when the connection is middling.
  ///
  /// In en, this message translates to:
  /// **'The picture may get smaller to keep moving.'**
  String get roomQualityMayShrink;

  /// A direct call this computer makes, while it rings: title; {name} is the friend being called.
  ///
  /// In en, this message translates to:
  /// **'Calling {name}…'**
  String roomCallingName(String name);

  /// A direct call this computer makes, while it rings: note under the title.
  ///
  /// In en, this message translates to:
  /// **'It rings on their computer for up to 30 seconds.'**
  String get roomRingsFor30;

  /// Button after a direct call ended for this computer while it goes on for the others: go back into it.
  ///
  /// In en, this message translates to:
  /// **'Return to call'**
  String get roomReturnToCall;

  /// Button after a direct call did not go through or ended: ring the friend again.
  ///
  /// In en, this message translates to:
  /// **'Call again'**
  String get roomCallAgain;

  /// Hint under this person's own tile while alone in a voice room.
  ///
  /// In en, this message translates to:
  /// **'When someone from the group comes in, they appear here.'**
  String get roomAloneHint;

  /// Hint under this person's own tile while alone in a voice room after others left; {names} is a list of names ("Marta and Leo").
  ///
  /// In en, this message translates to:
  /// **'{names} already left.'**
  String roomAlreadyLeft(String names);

  /// A step while connecting to a voice room: the group was found.
  ///
  /// In en, this message translates to:
  /// **'Group found on the network'**
  String get roomGroupFound;

  /// A step while connecting to a voice room, after a person's name and " · ": still looking for how to reach them. Lower case.
  ///
  /// In en, this message translates to:
  /// **'looking for the best route…'**
  String get roomLookingForRoute;

  /// Title of the panel shown while this computer connects to a voice room.
  ///
  /// In en, this message translates to:
  /// **'Connecting to the room'**
  String get roomConnectingTitle;

  /// Seconds spent connecting, short (like "3.4s"); {n} is the number.
  ///
  /// In en, this message translates to:
  /// **'{n}s'**
  String roomSeconds(String n);

  /// While connecting to a voice room: how many of the people inside are reached so far, and the seconds spent ({time} like "3.4s").
  ///
  /// In en, this message translates to:
  /// **'{reached} of {total} · {time}'**
  String roomReachedOf(int reached, int total, String time);

  /// How a person is reached: over the local network, encrypted. Lower case: it follows a name and " · ", or "Online · " (it is also shown with its first letter in capitals).
  ///
  /// In en, this message translates to:
  /// **'local network, encrypted'**
  String get roomPathLan;

  /// How a person is reached: a direct connection over the internet, encrypted. Lower case (see roomPathLan).
  ///
  /// In en, this message translates to:
  /// **'direct, encrypted'**
  String get roomPathDirect;

  /// How a person is reached: through another member of the group who passes the encrypted traffic on. Lower case (see roomPathLan).
  ///
  /// In en, this message translates to:
  /// **'through a member, encrypted'**
  String get roomPathThroughMember;

  /// How a person is reached: through the group member {name}, who passes the encrypted traffic on. Lower case (see roomPathLan).
  ///
  /// In en, this message translates to:
  /// **'through {name}, encrypted'**
  String roomPathThrough(String name);

  /// Title shown when connecting to a voice room failed.
  ///
  /// In en, this message translates to:
  /// **'Could not connect to the room'**
  String get roomCouldNotConnect;

  /// Text shown when connecting to a voice room failed.
  ///
  /// In en, this message translates to:
  /// **'Nobody answered in time. Your network may be blocking direct connections between devices.'**
  String get roomCouldNotConnectText;

  /// Beside each person's name in the list shown when connecting to a voice room failed: they did not answer. Short, lower case.
  ///
  /// In en, this message translates to:
  /// **'no answer'**
  String get roomNoAnswer;

  /// Link shown when connecting to a voice room failed: opens Settings › Network and its network check. Keep the arrow.
  ///
  /// In en, this message translates to:
  /// **'See the network check →'**
  String get roomSeeNetworkCheck;

  /// Above "Leave and close", when the last person leaves an empty voice room: what leaving does.
  ///
  /// In en, this message translates to:
  /// **'The room closes for everyone'**
  String get roomGoesNothing;

  /// Above "Leave and close", when the last person leaves a voice room: the chat messages that are lost when it closes.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 message goes} other{{n} messages go}}'**
  String roomGoesMessages(int n);

  /// Above "Leave and close", when the last person leaves a voice room: the files they shared that leave with them.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 file you shared goes} other{{n} files you shared go}}'**
  String roomGoesFiles(int n);

  /// Above "Leave and close", when the last person leaves a voice room: the chat messages and shared files that go when it closes.
  ///
  /// In en, this message translates to:
  /// **'{messages, plural, =1{1 message} other{{messages} messages}} and {files, plural, =1{1 file} other{{files} files}} go'**
  String roomGoesBoth(int messages, int files);

  /// Tooltip of the microphone button in a room's controls: turn it off.
  ///
  /// In en, this message translates to:
  /// **'Mute microphone'**
  String get roomMuteMicrophone;

  /// Tooltip of the microphone button in a room's controls: turn it on.
  ///
  /// In en, this message translates to:
  /// **'Unmute microphone'**
  String get roomUnmuteMicrophone;

  /// Tooltip of the screen button in a room's controls while sharing.
  ///
  /// In en, this message translates to:
  /// **'Stop sharing the screen'**
  String get roomStopSharingScreen;

  /// Tooltip of the screen button in a room's controls: pick a screen or window to share.
  ///
  /// In en, this message translates to:
  /// **'Share the screen'**
  String get roomShareScreen;

  /// Tooltip of the headphones button in a room's controls while deafened (nobody is heard): hear them again.
  ///
  /// In en, this message translates to:
  /// **'Hear the others again'**
  String get roomUndeafen;

  /// Tooltip of the headphones button in a room's controls: stop hearing everyone (and mute the microphone). The opposite of "Hear the others again"; plain words that say the incoming sound goes off, not the microphone.
  ///
  /// In en, this message translates to:
  /// **'Stop hearing the others'**
  String get roomDeafen;

  /// Tooltip of the camera button in a room's controls while the camera is on.
  ///
  /// In en, this message translates to:
  /// **'Turn the camera off'**
  String get roomCameraOff;

  /// Tooltip of the camera button in a room's controls while the camera is off.
  ///
  /// In en, this message translates to:
  /// **'Turn the camera on'**
  String get roomCameraOn;

  /// Red button in a room's controls: leave the voice room. Short.
  ///
  /// In en, this message translates to:
  /// **'Leave'**
  String get roomLeave;

  /// Tooltip of the screen-sound button in a room's controls: the shared screen's sound is only sent while the microphone is on.
  ///
  /// In en, this message translates to:
  /// **'The sound goes out with your microphone on'**
  String get roomSoundWaitsForMic;

  /// Tooltip of the screen-sound button in a room's controls: stop sending the screen's sound.
  ///
  /// In en, this message translates to:
  /// **'Stop sharing the sound'**
  String get roomStopSharingSound;

  /// Tooltip of the screen-sound button in a room's controls: also send the sound of the shared screen.
  ///
  /// In en, this message translates to:
  /// **'Share the sound too'**
  String get roomShareSoundToo;

  /// Tooltip of the button in a direct call's controls that opens the list of friends to add.
  ///
  /// In en, this message translates to:
  /// **'Add a friend to the call'**
  String get roomAddFriendToCall;

  /// Tooltip of the chat button in a room's controls: hide the chat and files beside the call.
  ///
  /// In en, this message translates to:
  /// **'Hide the chat'**
  String get roomHideChat;

  /// Tooltip of the chat button in a room's controls: show the chat and files beside the call.
  ///
  /// In en, this message translates to:
  /// **'Show the chat'**
  String get roomShowChat;

  /// Tab beside a room or call: the chat. Short.
  ///
  /// In en, this message translates to:
  /// **'Chat'**
  String get roomTabChat;

  /// Tab beside a room or call: the files shared in it. Short.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get roomTabFiles;

  /// Line in the chat of a direct call: {name} left it. A clock time (like 20:13) follows right after, so keep " · " and the trailing space at the end.
  ///
  /// In en, this message translates to:
  /// **'{name} left the call · '**
  String roomNoteLeftCall(String name);

  /// Line in the chat of a direct call: {name} joined it. A clock time (like 20:13) follows right after, so keep " · " and the trailing space at the end.
  ///
  /// In en, this message translates to:
  /// **'{name} joined the call · '**
  String roomNoteJoinedCall(String name);

  /// Line in the chat of a voice room: {name} left it. A clock time (like 20:13) follows right after, so keep " · " and the trailing space at the end.
  ///
  /// In en, this message translates to:
  /// **'{name} left the room · '**
  String roomNoteLeftRoom(String name);

  /// Line in the chat of a voice room: {name} came in. A clock time (like 20:13) follows right after, so keep " · " and the trailing space at the end.
  ///
  /// In en, this message translates to:
  /// **'{name} entered the room · '**
  String roomNoteEnteredRoom(String name);

  /// Placeholder and screen-reader label of the search field over a room's chat.
  ///
  /// In en, this message translates to:
  /// **'Search messages'**
  String get roomSearchMessages;

  /// Tooltip of the small x in the chat's search field.
  ///
  /// In en, this message translates to:
  /// **'Clear the search'**
  String get roomClearSearch;

  /// Note at the top of a direct call's chat: messages are not kept after the call.
  ///
  /// In en, this message translates to:
  /// **'This chat disappears when the call ends'**
  String get roomChatGoneCall;

  /// Note at the top of a voice room's chat: messages are not kept after the room closes.
  ///
  /// In en, this message translates to:
  /// **'This chat disappears when the room closes'**
  String get roomChatGoneRoom;

  /// Chat search found nothing; {text} is what was typed. Use the language's own quotation marks.
  ///
  /// In en, this message translates to:
  /// **'No messages with “{text}”'**
  String roomNoMatches(String text);

  /// Chat search: how many messages match.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 match} other{{n} matches}}'**
  String roomMatches(int n);

  /// Room chat while connecting: the messages said so far are being fetched.
  ///
  /// In en, this message translates to:
  /// **'Receiving the room’s messages…'**
  String get roomReceivingMessages;

  /// Room chat while connecting: the messages said so far are being fetched from the computer of {name}.
  ///
  /// In en, this message translates to:
  /// **'Receiving the room’s messages from {name}’s computer…'**
  String roomReceivingMessagesFrom(String name);

  /// Stands for a person whose name is not known (in who is typing, who shared a file).
  ///
  /// In en, this message translates to:
  /// **'Someone'**
  String get roomSomeone;

  /// Tooltip of the paperclip button in a room's message box; also the title of the empty Files tab.
  ///
  /// In en, this message translates to:
  /// **'Share files'**
  String get roomShareFiles;

  /// Tooltip of the paperclip button in a room's message box while files wait to be sent.
  ///
  /// In en, this message translates to:
  /// **'Add more files'**
  String get roomAddMoreFiles;

  /// Placeholder of the message box in a room or call: write to the group {name} (a group's name) or to the friend {name}.
  ///
  /// In en, this message translates to:
  /// **'Message {name}'**
  String roomMessageTo(String name);

  /// Placeholder of a voice room's message box when the group has no name to show.
  ///
  /// In en, this message translates to:
  /// **'Message the room'**
  String get roomMessageTheRoom;

  /// Placeholder of the message box in a direct call with several people.
  ///
  /// In en, this message translates to:
  /// **'Message the call'**
  String get roomMessageTheCall;

  /// Placeholder of the message box while files wait to be sent: a message may go with them.
  ///
  /// In en, this message translates to:
  /// **'Add a message (optional)'**
  String get roomAddMessageOptional;

  /// Screen-reader label of the message box in a room or call.
  ///
  /// In en, this message translates to:
  /// **'Message'**
  String get roomMessageLabel;

  /// Hint under a room's message box while files wait: pressing the key {key} (the Enter key, shown as a key cap) sends them.
  ///
  /// In en, this message translates to:
  /// **'{key} to send'**
  String roomEnterToSend(String key);

  /// Hint under a room's message box while files wait: pressing the key {key} (the Esc key, shown as a key cap) takes the waiting files out.
  ///
  /// In en, this message translates to:
  /// **'{key} to remove them'**
  String roomEscToRemove(String key);

  /// Two names as alternatives: "Marta or Leo" (who still has a file).
  ///
  /// In en, this message translates to:
  /// **'{first} or {second}'**
  String roomOrTwo(String first, String second);

  /// Three or more names as alternatives: {rest} is all but the last joined with the list separator ("Marta, Leo"), then the last one.
  ///
  /// In en, this message translates to:
  /// **'{rest} or {last}'**
  String roomOrMore(String rest, String last);

  /// Under a file in a room's chat that can no longer be fetched: {name}, who shared it, left the room.
  ///
  /// In en, this message translates to:
  /// **'Not available · {name} left'**
  String roomFileNotAvailable(String name);

  /// Under a file in a room: this computer shares it.
  ///
  /// In en, this message translates to:
  /// **'Sharing from this computer'**
  String get roomSharingFromHere;

  /// Under a file in a room: it can be fetched while the person who has it stays; {names} is one name, or several joined with "or" ("Marta or Leo").
  ///
  /// In en, this message translates to:
  /// **'Available while {names} stays in the room'**
  String roomAvailableWhile(String names);

  /// Under a file this person shared: its size ({size}, like "48.2 MB") and how many times others fetched it.
  ///
  /// In en, this message translates to:
  /// **'{size} · {n, plural, =1{1 download} other{{n} downloads}}'**
  String roomSizeDownloads(String size, int n);

  /// Tooltip of the download button on a file; {file} is its name.
  ///
  /// In en, this message translates to:
  /// **'Download {file}'**
  String roomDownloadFile(String file);

  /// Tooltip of the folder button on a file on this computer; {file} is its name.
  ///
  /// In en, this message translates to:
  /// **'Show {file} in its folder'**
  String roomShowInFolder(String file);

  /// A picture in the chat while it is being fetched; {file} is its file name.
  ///
  /// In en, this message translates to:
  /// **'Getting {file}…'**
  String roomGettingFile(String file);

  /// Tooltip of the button on a picture in the chat that saves it in the Downloads folder.
  ///
  /// In en, this message translates to:
  /// **'Save to Downloads'**
  String get roomSaveToDownloads;

  /// Under a picture in a room's chat whose sharer {name} left: this computer keeps it until leaving the room.
  ///
  /// In en, this message translates to:
  /// **'{name} left · the picture is kept here until you leave'**
  String roomPictureKept(String name);

  /// Button over a picture or video opened large: save the file in the Downloads folder. Short.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get roomSaveFile;

  /// Screen-reader label of the dark area around a picture opened large: closes it.
  ///
  /// In en, this message translates to:
  /// **'Close the picture'**
  String get roomClosePicture;

  /// Files tab of a room: note at the top on how sharing works (device to device, no server).
  ///
  /// In en, this message translates to:
  /// **'Files are not uploaded anywhere: they travel straight from the sharer’s computer, and only while they stay in the room.'**
  String get roomFilesNote;

  /// Files tab of a room: heading over the files that can be fetched, with how many there are.
  ///
  /// In en, this message translates to:
  /// **'Available · {n}'**
  String roomFilesAvailable(int n);

  /// Files tab of a room: heading over the files whose sharer left, with how many there are.
  ///
  /// In en, this message translates to:
  /// **'No longer available · {n}'**
  String roomFilesGone(int n);

  /// Files tab: under a file no longer available, its sharer {name} left the room (a clock time may follow after " · ").
  ///
  /// In en, this message translates to:
  /// **'{name} left'**
  String roomPersonLeft(String name);

  /// Files tab of a room: under "Share files", files can be picked here or dragged onto the room.
  ///
  /// In en, this message translates to:
  /// **'Pick them here or drop them on the room'**
  String get roomPickOrDrop;

  /// Files tab: under a file this person shared. Short; " · " and the size follow.
  ///
  /// In en, this message translates to:
  /// **'Yours'**
  String get roomFileYours;

  /// Files tab: under a file someone else shared; {name} is who. " · " and the size follow.
  ///
  /// In en, this message translates to:
  /// **'From {name}'**
  String roomFileFrom(String name);

  /// Files tab: how many times others fetched a file this person shared (after its size and " · ").
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 download} other{{n} downloads}}'**
  String roomDownloads(int n);

  /// Files tab: small button that stops sharing a file of this person. Short.
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get roomStopSharingFile;

  /// Title of the dialog that lists screens and windows to share.
  ///
  /// In en, this message translates to:
  /// **'What do you want to share?'**
  String get roomWhatToShare;

  /// A whole screen in the list of what to share; {name} is the system's name for it (like "Screen 1").
  ///
  /// In en, this message translates to:
  /// **'Screen: {name}'**
  String roomScreenSource(String name);

  /// Screen-reader label of a video in the chat: opens and plays it; {file} is its file name.
  ///
  /// In en, this message translates to:
  /// **'Play {file}'**
  String roomPlayVideo(String file);

  /// Screen-reader label of the dark area around a video opened large: closes it.
  ///
  /// In en, this message translates to:
  /// **'Close the video'**
  String get roomCloseVideo;

  /// Shown in place of a video opened large when its sharer left the room.
  ///
  /// In en, this message translates to:
  /// **'This video is no longer in the room.'**
  String get roomVideoGone;

  /// Shown in place of a video opened large when it could not be fetched.
  ///
  /// In en, this message translates to:
  /// **'Could not get this video.'**
  String get roomVideoNotFetched;

  /// Shown in place of a video opened large when the system's player cannot play its format.
  ///
  /// In en, this message translates to:
  /// **'This computer cannot play this video.'**
  String get roomVideoCannotPlay;

  /// A video opened large while it is fetched, with how much has come (a percent).
  ///
  /// In en, this message translates to:
  /// **'Getting the video · {n}%'**
  String roomGettingVideoPercent(int n);

  /// A video opened large while it is fetched.
  ///
  /// In en, this message translates to:
  /// **'Getting the video…'**
  String get roomGettingVideo;

  /// Video player: screen-reader label of the button that pauses.
  ///
  /// In en, this message translates to:
  /// **'Pause'**
  String get roomPause;

  /// Video player: screen-reader label of the button that plays.
  ///
  /// In en, this message translates to:
  /// **'Play'**
  String get roomPlay;

  /// Video player: tooltip of the speaker button while muted (turn the sound on).
  ///
  /// In en, this message translates to:
  /// **'Sound on'**
  String get roomSoundOn;

  /// Video player: tooltip of the speaker button (turn the sound off).
  ///
  /// In en, this message translates to:
  /// **'Mute'**
  String get roomMuteVideo;

  /// Video player: tooltip of the button that leaves full screen.
  ///
  /// In en, this message translates to:
  /// **'Leave full screen'**
  String get roomLeaveFullScreen;

  /// Pill in a direct call after someone left: how long the call stays open for the others; {time} is a countdown like "1:00".
  ///
  /// In en, this message translates to:
  /// **'When only one is left, the call waits {time} for the others to come back'**
  String roomCallWaits(String time);

  /// Pill in a direct call when this person is the only one left: how long the call stays open; {time} is a countdown like "0:42".
  ///
  /// In en, this message translates to:
  /// **'You’re the only one left. The call waits {time} for the others to come back'**
  String roomCallWaitsAlone(String time);

  /// Screen-reader label of a picture in the chat: opens it large; {file} is its file name.
  ///
  /// In en, this message translates to:
  /// **'Open {file}'**
  String roomOpenFile(String file);

  /// Group settings › Activity: one line of the group's history, saying who did what. {name} is the group's first name. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} started the group as “{name}”'**
  String activityCreated(String by, String byMe, String name);

  /// Group settings › Activity: one line of the group's history, saying who did what. {name} is the group's new name. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} renamed the group “{name}”'**
  String activityRenamed(String by, String byMe, String name);

  /// Group settings › Activity: one line of the group's history, saying who did what. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} changed the group’s icon'**
  String activityIcon(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} removed the group’s picture'**
  String activityPictureRemoved(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} changed the group’s picture'**
  String activityPicture(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. The banner is the wide picture at the top of the group. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} removed the group’s banner'**
  String activityBannerRemoved(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. The banner is the wide picture at the top of the group. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} changed the group’s banner'**
  String activityBanner(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. Someone added a friend to the group. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} added {memberMe, select, yes{you} other{{member}}}'**
  String activityAdded(String by, String byMe, String member, String memberMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. Someone joined with an invite link made by {member}. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} joined with an invite from {memberMe, select, yes{you} other{{member}}}'**
  String activityJoined(String by, String byMe, String member, String memberMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. A manager accepted someone who asked to join. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} let {memberMe, select, yes{you} other{{member}}} in'**
  String activityLetIn(String by, String byMe, String member, String memberMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. A manager removed someone from the group. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} removed {memberMe, select, yes{you} other{{member}}}'**
  String activityRemoved(
    String by,
    String byMe,
    String member,
    String memberMe,
  );

  /// Group settings › Activity: one line of the group's history, saying who did what. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} left the group'**
  String activityLeft(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. Members are in an order (the first ones manage the group). {place} is the new place, already written as an ordinal like "2nd". {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} moved {memberMe, select, yes{you} other{{member}}} up to {place}'**
  String activityMovedUp(
    String by,
    String byMe,
    String member,
    String memberMe,
    String place,
  );

  /// Group settings › Activity: one line of the group's history, saying who did what. Members are in an order; {member} was moved to the end of it. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} moved {memberMe, select, yes{you} other{{member}}} to the end'**
  String activityMovedDown(
    String by,
    String byMe,
    String member,
    String memberMe,
  );

  /// Group settings › Activity: one line of the group's history, saying who did what. Everyone in the group may now manage it. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} let everyone manage the group'**
  String activityManagersAll(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. Only the first {n} members in the group's order manage it now. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} left managing the group to {n, plural, =1{the first one only} other{the first {n}}}'**
  String activityManagersFirst(String by, String byMe, int n);

  /// Group settings › Activity: one line of the group's history, saying who did what. The most people the group and each of its voice rooms take. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} set the group to {people} people and {perRoom} in each voice room'**
  String activityLimits(String by, String byMe, int people, int perRoom);

  /// Group settings › Activity: one line of the group's history, saying who did what. {room} is the voice room's name. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} opened the voice room “{room}”'**
  String activityRoomOpened(String by, String byMe, String room);

  /// Group settings › Activity: one line of the group's history, saying who did what. {from} is the old name, {room} the new one. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} renamed the voice room “{from}” to “{room}”'**
  String activityRoomRenamed(String by, String byMe, String from, String room);

  /// Group settings › Activity: one line of the group's history, saying who did what. The voice room was deleted. {room} is its name. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} deleted the voice room “{room}”'**
  String activityRoomClosed(String by, String byMe, String room);

  /// Group settings › Activity: one line of the group's history, saying who did what. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} made an invite link'**
  String activityInvited(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} cancelled an invite link'**
  String activityRevoked(String by, String byMe);

  /// Group settings › Activity: one line of the group's history, saying who did what. Everyone in the group may now enter the voice room {room}. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} opened “{room}” to everyone'**
  String activityRoomOpenToAll(String by, String byMe, String room);

  /// Group settings › Activity: one line of the group's history, saying who did what. Only members above the one who did it in the group's order may enter the voice room {room}. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} kept “{room}” for those above'**
  String activityRoomForAbove(String by, String byMe, String room);

  /// Group settings › Activity: one line of the group's history, saying who did what. Only the people listed may enter the voice room {room}. {people} is a list of names joined with the list separator (it may contain the word for "you"). {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} let only {people} into “{room}”'**
  String activityRoomOnly(String by, String byMe, String people, String room);

  /// Group settings › Activity: one line of the group's history, saying who did what. A manager kept someone out of the voice room {room} with no end time. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} kept {memberMe, select, yes{you} other{{member}}} out of “{room}” until let back'**
  String activityKeptOut(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  );

  /// Group settings › Activity: one line of the group's history, saying who did what. A manager kept someone out of the voice room {room} until {until}, a day and time like "Oct 21, 23:23" or "yesterday 23:23". {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'. {untilKind} says how {until} is written, for the words before it: "today" (only a clock time), "todayAtOne" (only a clock time whose hour is 1, as in 1:05; Spanish says "hasta la 1:05", not "las"), "yesterday", or "other" (a day of the year: French "jusqu’au", Italian "fino al giorno"). English says "until" in every case.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} timed {memberMe, select, yes{you} other{{member}}} out of “{room}” {untilKind, select, today{until {until}} todayAtOne{until {until}} yesterday{until {until}} other{until {until}}}'**
  String activityTimedOut(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
    String until,
    String untilKind,
  );

  /// Group settings › Activity: one line of the group's history, saying who did what. A manager let someone back into the voice room {room}. {by} is the name of the person who did it; {byMe} is 'yes' when that is the person using this computer (English then says "You"), else 'no'. {member} is the person it was done to; {memberMe} is 'yes' when that is the person using this computer (English then says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{byMe, select, yes{You} other{{by}}} let {memberMe, select, yes{you} other{{member}}} back into “{room}”'**
  String activityLetBack(
    String by,
    String byMe,
    String member,
    String memberMe,
    String room,
  );

  /// Name shown for a voice room that has no name of its own (the group's first voice room). Short.
  ///
  /// In en, this message translates to:
  /// **'General'**
  String get activityDefaultRoom;

  /// The word for "you" (the person using this computer) inside a list of names in a group activity line: "Ana let only Leo, you into “General”". Lower case.
  ///
  /// In en, this message translates to:
  /// **'you'**
  String get activityYouInList;

  /// Group settings › Activity: why a line of the group's history was undone, shown under it. {author} is who did the undone action; {authorMe} is 'yes' when that is the person using this computer (English says "you"), else 'no'. {by} is who undid it; {byWho} is 'me' (the person using this computer), 'other' (then {by} is a name), or 'none' (nobody named: say nothing about who). The author had been removed from the group at the same moment.
  ///
  /// In en, this message translates to:
  /// **'Undone: {authorMe, select, yes{you} other{{author}}} had been removed{byWho, select, me{ by you} other{ by {by}} none{}} at the same time'**
  String activityUndoneRemoved(
    String author,
    String authorMe,
    String by,
    String byWho,
  );

  /// Group settings › Activity: why a line of the group's history was undone, shown under it. {author} is who did the undone action; {authorMe} is 'yes' when that is the person using this computer (English says "you"), else 'no'. {by} is who undid it; {byWho} is 'me' (the person using this computer), 'other' (then {by} is a name), or 'none' (nobody named: say nothing about who). The author had been moved to the end of the group's order at the same moment.
  ///
  /// In en, this message translates to:
  /// **'Undone: {authorMe, select, yes{you} other{{author}}} had been moved to the end{byWho, select, me{ by you} other{ by {by}} none{}} at the same time'**
  String activityUndoneMovedDown(
    String author,
    String authorMe,
    String by,
    String byWho,
  );

  /// Group settings › Activity: why a line was undone: its author left the group at the same moment. {author} is a name; {authorMe} is 'yes' when that is the person using this computer (English says "you"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'Undone: {authorMe, select, yes{you} other{{author}}} had left at the same time'**
  String activityUndoneLeft(String author, String authorMe);

  /// Group settings › Activity: note above the group's history.
  ///
  /// In en, this message translates to:
  /// **'Everyone in the group sees the same list, from the group’s own record. Changes to the picture and banner are kept apart: those are the ones this device saw.'**
  String get activityAboutList;

  /// Group settings › Activity: note that lines are in the group's own order and times come from each author's computer clock.
  ///
  /// In en, this message translates to:
  /// **'In the group’s own order; times are each author’s clock.'**
  String get activityAboutOrder;

  /// Group settings › Activity: shown when the group's history is empty.
  ///
  /// In en, this message translates to:
  /// **'Nothing yet.'**
  String get activityNothingYet;

  /// Button text while older lines of the history load.
  ///
  /// In en, this message translates to:
  /// **'Loading…'**
  String get activityLoading;

  /// Button: load older lines of the group history.
  ///
  /// In en, this message translates to:
  /// **'Show older'**
  String get activityShowOlder;

  /// Under a line of the group history: when it happened ({when}, like "Oct 21, 23:23"), and that this computer saw it itself.
  ///
  /// In en, this message translates to:
  /// **'{when} · seen on this device'**
  String activitySeenHere(String when);

  /// Tooltip and title of the bell button and its panel in the title bar.
  ///
  /// In en, this message translates to:
  /// **'Notifications'**
  String get bellNotifications;

  /// Screen-reader label of the bell button when there are {n} new notifications.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 new notification} other{{n} new notifications}}'**
  String bellNewNotifications(int n);

  /// Bell panel: small button that dismisses every notification.
  ///
  /// In en, this message translates to:
  /// **'Clear all'**
  String get bellClearAll;

  /// Bell panel: shown when there are no notifications.
  ///
  /// In en, this message translates to:
  /// **'Nothing new'**
  String get bellNothingNew;

  /// Bell panel: note at the bottom. "Friends" is the Friends page.
  ///
  /// In en, this message translates to:
  /// **'The last three stay here. Missed calls and requests also show in Friends.'**
  String get bellFooter;

  /// Bell, missed call notice: the friend who called and you are in the same voice room {room} (its name), so there is no call button.
  ///
  /// In en, this message translates to:
  /// **'You’re both in {room}.'**
  String bellBothInRoom(String room);

  /// Bell: button that removes one notification. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Dismiss'**
  String get bellDismiss;

  /// The bell (notifications panel in the title bar): title of a friend request. {name} is a person's name.
  ///
  /// In en, this message translates to:
  /// **'{name} wants to be your friend'**
  String bellFriendRequest(String name);

  /// The bell (notifications panel in the title bar): under a friend request. {fingerprint} is a short code like C07N·50S4 identifying the computer; keep it as is.
  ///
  /// In en, this message translates to:
  /// **'Check that the fingerprint matches: {fingerprint}'**
  String bellCheckFingerprint(String fingerprint);

  /// The bell (notifications panel in the title bar): someone used an invite link and waits for a manager to let them in. {name} is a person, {group} a group name.
  ///
  /// In en, this message translates to:
  /// **'{name} asks to join “{group}”'**
  String bellJoinAsk(String name, String group);

  /// The bell (notifications panel in the title bar): under a request to join. {fingerprint} is a short code identifying the computer; keep it as is.
  ///
  /// In en, this message translates to:
  /// **'With one of your group’s invite links. Fingerprint: {fingerprint}'**
  String bellJoinAskNote(String fingerprint);

  /// Bell: button that accepts someone into the group. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Let in'**
  String get bellLetIn;

  /// Bell: button that refuses someone who asked to join. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Say no'**
  String get bellSayNo;

  /// The bell (notifications panel in the title bar): a friend added you to a group; you can join or answer later. {name} is the friend, {group} the group.
  ///
  /// In en, this message translates to:
  /// **'{name} added you to “{group}”'**
  String bellGroupInvite(String name, String group);

  /// The bell (notifications panel in the title bar): under a group invitation: how many people are in the group.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 member} other{{n} members}}'**
  String bellMembers(int n);

  /// Bell: button to join a group you were invited to. Short.
  ///
  /// In en, this message translates to:
  /// **'Join'**
  String get bellJoin;

  /// The bell (notifications panel in the title bar): unread direct messages from a friend. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{New message from {name}} other{{n} new messages from {name}}}'**
  String bellNewMessages(int n, String name);

  /// Bell: button that opens the direct chat. Short.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get bellOpen;

  /// Bell: button that marks the messages as read. Short.
  ///
  /// In en, this message translates to:
  /// **'Mark as read'**
  String get bellMarkRead;

  /// The bell (notifications panel in the title bar): title of the notice about this computer's firewall.
  ///
  /// In en, this message translates to:
  /// **'Your firewall keeps some friends out'**
  String get bellFirewallTitle;

  /// Bell: button that copies the firewall command. Short.
  ///
  /// In en, this message translates to:
  /// **'Copy command'**
  String get bellCopyCommand;

  /// The bell (notifications panel in the title bar): title of the notice that the same identity is open on another computer at the same time.
  ///
  /// In en, this message translates to:
  /// **'Open on another computer too'**
  String get bellOtherSessionTitle;

  /// The bell (notifications panel in the title bar): title of the notice, given once at start, that the settings file was damaged and what could not be read went back to its default.
  ///
  /// In en, this message translates to:
  /// **'Some settings were reset'**
  String get bellSettingsResetTitle;

  /// The bell: text of that notice. app.json.bad is a file name: keep it as it is. Settings is the app's Settings screen.
  ///
  /// In en, this message translates to:
  /// **'Some of your settings could not be read and are back to their defaults: check them in Settings. The old file was kept beside the new one, as app.json.bad.'**
  String get bellSettingsResetText;

  /// Friend card (a friend's profile card, opened from Friends or a direct chat): tooltip of the status dot: the friend is online.
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get friendCardOnline;

  /// Friend card (a friend's profile card, opened from Friends or a direct chat): tooltip of the status dot: the friend is online but has not used their computer for 10 minutes or more.
  ///
  /// In en, this message translates to:
  /// **'Away'**
  String get friendCardAway;

  /// Friend card (a friend's profile card, opened from Friends or a direct chat): tooltip of the status dot: the friend is offline.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get friendCardOffline;

  /// Friend card (a friend's profile card, opened from Friends or a direct chat): note under the disabled Call button.
  ///
  /// In en, this message translates to:
  /// **'Calls ring only while a friend is online.'**
  String get friendCardCallsOnlyOnline;

  /// Friend card (a friend's profile card, opened from Friends or a direct chat): note under the disabled Call button while in another call. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'Hang up first to call {name}.'**
  String friendCardHangUpFirst(String name);

  /// Friend card (a friend's profile card, opened from Friends or a direct chat): note under the Call button: the friend and you are in the same voice room {room} (its name), so no call is needed.
  ///
  /// In en, this message translates to:
  /// **'You’re both in {room}.'**
  String friendCardBothInRoom(String room);

  /// Friend card (a friend's profile card, opened from Friends or a direct chat): button that opens a direct chat with the friend (verb). Short.
  ///
  /// In en, this message translates to:
  /// **'Message'**
  String get friendCardMessage;

  /// Friend card (a friend's profile card, opened from Friends or a direct chat): how many friends you and this friend have in common.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 mutual friend} other{{n} mutual friends}}'**
  String friendCardMutualFriends(int n);

  /// Member card (the card of a group member who is not your friend, opened by clicking them in a group): button that sends them a friend request, no link or code needed. Short.
  ///
  /// In en, this message translates to:
  /// **'Send friend request'**
  String get memberCardSendRequest;

  /// Member card (the card of a group member who is not your friend): shown instead of the button once you sent them a friend request that waits for their answer.
  ///
  /// In en, this message translates to:
  /// **'Friend request sent'**
  String get memberCardRequestSent;

  /// Member card (the card of a group member who is not your friend): button that accepts the friend request this member sent you. Short.
  ///
  /// In en, this message translates to:
  /// **'Accept friend request'**
  String get memberCardAccept;

  /// Member card (the card of a group member who is not your friend): note above the Accept button: this member sent you a friend request. {name} is the member's name.
  ///
  /// In en, this message translates to:
  /// **'{name} wants to be your friend.'**
  String memberCardAsksYou(String name);

  /// Member card (the card of a group member who is not your friend): line under the name: the group it was opened from. {group} is the group's name.
  ///
  /// In en, this message translates to:
  /// **'Member of {group}'**
  String memberCardInGroup(String group);

  /// Member card (the card of a group member who is not your friend): note under the disabled Send friend request button: the request can only go while the member is online. {name} is the member's name.
  ///
  /// In en, this message translates to:
  /// **'Friend requests go only while {name} is online.'**
  String memberCardOnlyOnline(String name);

  /// What screen readers say for your own name or picture in a list or a chat: clicking it shows your own card (the panel with your profile and status).
  ///
  /// In en, this message translates to:
  /// **'Show your card'**
  String get memberCardShowOwn;

  /// Your own panel (opened by clicking your pill at the bottom left): small button that opens Settings › My profile.
  ///
  /// In en, this message translates to:
  /// **'Edit profile'**
  String get userPanelEditProfile;

  /// Your own panel (opened by clicking your pill at the bottom left): label before the Online / Offline choice.
  ///
  /// In en, this message translates to:
  /// **'Status'**
  String get userPanelStatus;

  /// Your own panel (opened by clicking your pill at the bottom left): status choice and tooltip: friends see you online. Short: fits a small segmented button.
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get userPanelOnline;

  /// Your own panel (opened by clicking your pill at the bottom left): tooltip of your status dot while friends and members of your groups see you as away, because nobody used this computer for 10 minutes. Not a choice: it comes by itself.
  ///
  /// In en, this message translates to:
  /// **'Away'**
  String get userPanelAway;

  /// Your own panel (opened by clicking your pill at the bottom left): status choice: appear offline to friends. Short: fits a small segmented button.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get userPanelOffline;

  /// Your own panel (opened by clicking your pill at the bottom left): note under the status while appearing offline.
  ///
  /// In en, this message translates to:
  /// **'Friends and members of your groups see you as offline. You can still be in rooms.'**
  String get userPanelOfflineNote;

  /// Your own panel (opened by clicking your pill at the bottom left): note under the status while online.
  ///
  /// In en, this message translates to:
  /// **'Friends and members of your groups see you online, or away after 10 min without using this computer.'**
  String get userPanelOnlineNote;

  /// Heading (in capitals in English) above the person's bio (a short text about themselves) on a profile card. Short.
  ///
  /// In en, this message translates to:
  /// **'BIO'**
  String get userPanelBio;

  /// Your own panel (opened by clicking your pill at the bottom left): tooltip of your status dot while you appear offline to friends.
  ///
  /// In en, this message translates to:
  /// **'Appearing offline'**
  String get userPanelAppearingOffline;

  /// Your own panel (opened by clicking your pill at the bottom left): screen-reader label of the row that copies a new friend link.
  ///
  /// In en, this message translates to:
  /// **'Copy a friend link'**
  String get userPanelCopyLinkLabel;

  /// Your own panel (opened by clicking your pill at the bottom left): how long the copied friend link still works; {time} is a countdown like 9:41 (minutes:seconds).
  ///
  /// In en, this message translates to:
  /// **'Expires in {time}'**
  String userPanelLinkExpiresIn(String time);

  /// Your own panel (opened by clicking your pill at the bottom left): small note beside "Copy friend link": a link works for one person and for 10 minutes. Very short.
  ///
  /// In en, this message translates to:
  /// **'1 person · 10 min'**
  String get userPanelLinkTerms;

  /// Your own panel (opened by clicking your pill at the bottom left): row that copies a new friend link to send to someone.
  ///
  /// In en, this message translates to:
  /// **'Copy friend link'**
  String get userPanelCopyLink;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): detail beside a direct chat with someone who is no longer a friend.
  ///
  /// In en, this message translates to:
  /// **'Direct chat · no longer friends'**
  String get switcherChatEnded;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): detail beside a direct chat whose friend is online.
  ///
  /// In en, this message translates to:
  /// **'Direct chat · online'**
  String get switcherChatOnline;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): detail beside a direct chat whose friend is online but has not used their computer for 10 minutes or more.
  ///
  /// In en, this message translates to:
  /// **'Direct chat · away'**
  String get switcherChatAway;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): detail beside a direct chat.
  ///
  /// In en, this message translates to:
  /// **'Direct chat'**
  String get switcherChat;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): detail beside a friend without a chat yet. {presence} is their status: "Online", "Offline" or "Offline · seen 5 min ago".
  ///
  /// In en, this message translates to:
  /// **'Friend · {presence}'**
  String switcherFriend(String presence);

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): detail beside a voice room. {group} is its group's name.
  ///
  /// In en, this message translates to:
  /// **'Voice room in {group}'**
  String switcherVoiceRoom(String group);

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): detail beside a voice room with {n} people in it now (1 or more). {group} is its group's name.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, other{Voice room in {group} · {n} in it}}'**
  String switcherVoiceRoomPeople(String group, int n);

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): detail beside a group with {n} members.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{Group · 1 member} other{Group · {n} members}}'**
  String switcherGroup(int n);

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): placeholder text in the search box.
  ///
  /// In en, this message translates to:
  /// **'Go to a chat, group or voice room'**
  String get switcherPlaceholder;

  /// Quick switcher: screen-reader label of the search box and of the round button in the title bar that opens it. Short.
  ///
  /// In en, this message translates to:
  /// **'Go to'**
  String get switcherGoTo;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): shown when there is nothing to list.
  ///
  /// In en, this message translates to:
  /// **'No groups, friends or chats yet.'**
  String get switcherEmpty;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): nothing matches the text typed ({query}).
  ///
  /// In en, this message translates to:
  /// **'Nothing matches “{query}”.'**
  String switcherNoMatch(String query);

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): keyboard hint at the bottom shown after the arrow keys "↑ ↓": they choose a line. Short, lower case.
  ///
  /// In en, this message translates to:
  /// **'to choose'**
  String get switcherHintChoose;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): keyboard hint shown after the key "Enter": it goes to the chosen place. Short, lower case.
  ///
  /// In en, this message translates to:
  /// **'to go'**
  String get switcherHintGo;

  /// Quick switcher (Ctrl K: a search box to go to a chat, friend, voice room or group): keyboard hint shown after the key "Esc": it closes the switcher. Short, lower case.
  ///
  /// In en, this message translates to:
  /// **'to close'**
  String get switcherHintClose;

  /// Tooltip of the round button in the title bar that opens the quick switcher. {shortcut} is the keyboard shortcut, like "Ctrl K" or "⌘K".
  ///
  /// In en, this message translates to:
  /// **'Go to a chat, group or voice room ({shortcut})'**
  String switcherTooltip(String shortcut);

  /// Friends page: line under the title.
  ///
  /// In en, this message translates to:
  /// **'Each friendship lives only on both devices. No directory, no server.'**
  String get friendsIntro;

  /// Friends page: tab listing every friend (a count shows beside it). Short.
  ///
  /// In en, this message translates to:
  /// **'All'**
  String get friendsTabAll;

  /// Friends page: tab listing friends online now (a count shows beside it). Short.
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get friendsTabOnline;

  /// Friends page: tab listing friend requests (a count shows beside it). Short.
  ///
  /// In en, this message translates to:
  /// **'Requests'**
  String get friendsTabRequests;

  /// Friends page: placeholder and label of the search field.
  ///
  /// In en, this message translates to:
  /// **'Search friends'**
  String get friendsSearch;

  /// Friends page: empty list. QR, link and code are the three ways to add a friend.
  ///
  /// In en, this message translates to:
  /// **'No friends yet. Share your QR, link or code, or use someone else’s.'**
  String get friendsNone;

  /// Friends page: the Online tab is empty.
  ///
  /// In en, this message translates to:
  /// **'No friend is online right now.'**
  String get friendsNoneOnline;

  /// Friends page: the search finds nobody.
  ///
  /// In en, this message translates to:
  /// **'Nobody matches the search.'**
  String get friendsNoMatch;

  /// A friend's status: they are in a voice room (which one is not known here).
  ///
  /// In en, this message translates to:
  /// **'In a room'**
  String get friendsInARoom;

  /// A friend's status: they are in a voice room of the group {group}.
  ///
  /// In en, this message translates to:
  /// **'In room · {group}'**
  String friendsInRoomOf(String group);

  /// A friend's status: online.
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get friendsOnline;

  /// A friend's status: online, but they have not used their computer for 10 minutes or more.
  ///
  /// In en, this message translates to:
  /// **'Away'**
  String get friendsAway;

  /// A friend's status: offline, never seen online.
  ///
  /// In en, this message translates to:
  /// **'Not seen online yet'**
  String get friendsNotSeenYet;

  /// A friend's status: offline; {when} is when they were last online, like "5 min ago", "yesterday" or "3 days ago".
  ///
  /// In en, this message translates to:
  /// **'Seen {when}'**
  String friendsSeen(String when);

  /// Friends page: screen-reader label of a friend's row; it opens their profile card. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'Show {name}’s card'**
  String friendsShowCard(String name);

  /// Friends page: under a friend's name: a call from them was missed; {when} is a time like "23:23" or "yesterday 23:23".
  ///
  /// In en, this message translates to:
  /// **'Missed call · {when}'**
  String friendsMissedCall(String when);

  /// Friends page: button on a friend's row that opens a direct chat (verb). Short.
  ///
  /// In en, this message translates to:
  /// **'Message'**
  String get friendsMessage;

  /// Friends page: tooltip of the disabled Call button. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'{name} is offline: calls ring only while a friend is online'**
  String friendsOfflineNoCall(String name);

  /// Friends page: tooltip of the disabled Call button during another call. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'Hang up first to call {name}'**
  String friendsHangUpFirst(String name);

  /// Friends page: tooltip of the disabled Call button: the friend and you are in the same voice room {room} (its name).
  ///
  /// In en, this message translates to:
  /// **'You’re both in {room}.'**
  String friendsBothInRoom(String room);

  /// Friends page: tooltip of the "···" button on a friend's row, and the label of its menu. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'Options for {name}'**
  String friendsOptions(String name);

  /// Friends page: title of the dialog that asks before removing a friend.
  ///
  /// In en, this message translates to:
  /// **'Remove {name} from your friends?'**
  String friendsRemoveTitle(String name);

  /// Friends page: text of the dialog that asks before removing a friend.
  ///
  /// In en, this message translates to:
  /// **'You disappear from each other’s friends, without a message. You stay together in the groups you share. Becoming friends again takes another invitation.'**
  String get friendsRemoveText;

  /// Friends page: danger button in the dialog: removes the friend.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get friendsRemove;

  /// Friends page: item of a friend's "···" menu: add them to the group {group}.
  ///
  /// In en, this message translates to:
  /// **'Add to “{group}”'**
  String friendsAddToGroup(String group);

  /// Friends page: item of a friend's "···" menu: remove them (asks first).
  ///
  /// In en, this message translates to:
  /// **'Remove from friends…'**
  String get friendsRemoveMenu;

  /// Friends page: the Requests tab is empty.
  ///
  /// In en, this message translates to:
  /// **'No pending requests.'**
  String get friendsNoRequests;

  /// Friends page: a friend request to answer; {when} is like "5 min ago".
  ///
  /// In en, this message translates to:
  /// **'Wants to be your friend · {when}'**
  String friendsRequestIncoming(String when);

  /// Friends page: a friend request this person sent; {when} is like "5 min ago".
  ///
  /// In en, this message translates to:
  /// **'Sent {when} · waiting for an answer'**
  String friendsRequestOutgoing(String when);

  /// Friends page: small heading of the card with a friend request to answer.
  ///
  /// In en, this message translates to:
  /// **'Pending request'**
  String get friendsPendingRequest;

  /// Friends page: on a friend request: compare the code shown with the one on the other person's screen. {name} is their name.
  ///
  /// In en, this message translates to:
  /// **'Check that the fingerprint matches the one {name} sees on their screen.'**
  String friendsCheckFingerprint(String name);

  /// Friends page: title of the panel with your QR, link and code.
  ///
  /// In en, this message translates to:
  /// **'Add a friend'**
  String get friendsAddTitle;

  /// Friends page: line under "Add a friend".
  ///
  /// In en, this message translates to:
  /// **'Your devices exchange keys directly.'**
  String get friendsAddNote;

  /// Friends page: tab: a short code to read out. Short.
  ///
  /// In en, this message translates to:
  /// **'Code'**
  String get friendsTabCode;

  /// Friends page: tab: a link to send. Short.
  ///
  /// In en, this message translates to:
  /// **'Link'**
  String get friendsTabLink;

  /// Friends page: tab: a QR code to scan. Short (usually stays "QR").
  ///
  /// In en, this message translates to:
  /// **'QR'**
  String get friendsTabQr;

  /// Friends page: button that makes a new friend invitation (QR, link or code).
  ///
  /// In en, this message translates to:
  /// **'Make an invitation'**
  String get friendsMakeInvitation;

  /// Friends page: the invitation shown is too old.
  ///
  /// In en, this message translates to:
  /// **'This invitation has expired.'**
  String get friendsInvitationExpired;

  /// Friends page: button that makes a new invitation after one expired.
  ///
  /// In en, this message translates to:
  /// **'Make another'**
  String get friendsMakeAnother;

  /// Friends page: under the QR code. {time} is a countdown like 9:41 (minutes:seconds), drawn in another font; it may go anywhere in the sentence.
  ///
  /// In en, this message translates to:
  /// **'Scan it from Fogata · expires in {time}'**
  String friendsHintQr(String time);

  /// Friends page: under the link. {time} is a countdown like 9:41 (minutes:seconds), drawn in another font; it may go anywhere in the sentence.
  ///
  /// In en, this message translates to:
  /// **'Send it to one person · expires in {time}'**
  String friendsHintLink(String time);

  /// Friends page: under the code. {time} is a countdown like 9:41 (minutes:seconds), drawn in another font; it may go anywhere in the sentence.
  ///
  /// In en, this message translates to:
  /// **'Read it out to one person · expires in {time}'**
  String friendsHintCode(String time);

  /// Friends page: the code is blurred until shown, so it does not leak on a shared screen.
  ///
  /// In en, this message translates to:
  /// **'Hidden in case you share your screen.'**
  String get friendsHiddenCode;

  /// Friends page: the link or QR is blurred until shown; it also contains network addresses.
  ///
  /// In en, this message translates to:
  /// **'Hidden in case you share your screen. It carries this computer’s network addresses: share it in private only.'**
  String get friendsHiddenLink;

  /// Friends page: heading above the field where a link or code someone gave you is pasted.
  ///
  /// In en, this message translates to:
  /// **'Got a friend or group link, or a code?'**
  String get friendsGotLink;

  /// Friends page: placeholder of the field for a friend or group link, or a code.
  ///
  /// In en, this message translates to:
  /// **'Paste a link or code'**
  String get friendsPastePlaceholder;

  /// Friends page: screen-reader label of the field for a link or code.
  ///
  /// In en, this message translates to:
  /// **'Link or code'**
  String get friendsLinkOrCode;

  /// Friends page: button that uses the pasted link or code. Short.
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get friendsAdd;

  /// Screen-reader label of the QR code image.
  ///
  /// In en, this message translates to:
  /// **'QR code of your invitation'**
  String get friendsQrLabel;

  /// Short note after adding a friend to a group. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'{name} gets the group the next time you are both online.'**
  String friendsAddedToGroup(String name);

  /// Direct chat: day heading between messages, and the day in a pinned message's time.
  ///
  /// In en, this message translates to:
  /// **'Today'**
  String get directToday;

  /// Direct chat: day heading between messages.
  ///
  /// In en, this message translates to:
  /// **'Yesterday'**
  String get directYesterday;

  /// Direct chat: line in the chat when the disappearing-messages timer was turned off. {name} is the friend's name; {mine} is 'yes' when the person using this computer did it (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{mine, select, yes{You} other{{name}}} turned off disappearing messages'**
  String directTimerOff(String name, String mine);

  /// Direct chat: first line of a new chat that starts with disappearing messages; {time} is how long messages last, like "1 day" or "1 week".
  ///
  /// In en, this message translates to:
  /// **'Disappearing messages on: {time}'**
  String directTimerFirst(String time);

  /// Direct chat: line in the chat when the disappearing-messages timer was set; {time} is how long messages last, like "1 day". {name} is the friend's name; {mine} is 'yes' when the person using this computer did it (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{mine, select, yes{You} other{{name}}} turned on disappearing messages: {time}'**
  String directTimerOn(String name, String mine, String time);

  /// Direct chat: line in the chat when a message was pinned. {name} is the friend's name; {mine} is 'yes' when the person using this computer did it (English then says "You"), else 'no'.
  ///
  /// In en, this message translates to:
  /// **'{mine, select, yes{You} other{{name}}} pinned a message'**
  String directPinnedLine(String name, String mine);

  /// Direct chat: how long a call lasted, under a minute. Lower case: follows "lasted".
  ///
  /// In en, this message translates to:
  /// **'a few seconds'**
  String get directCallFewSeconds;

  /// Direct chat: how long a call lasted. Lower case: follows "lasted".
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 minute} other{{n} minutes}}'**
  String directCallMinutes(int n);

  /// Direct chat: how long a call lasted. Lower case: follows "lasted".
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 hour} other{{n} hours}}'**
  String directCallHours(int n);

  /// Direct chat: how long a call lasted, hours and minutes. Lower case: follows "lasted".
  ///
  /// In en, this message translates to:
  /// **'{hours, plural, =1{1 hour} other{{hours} hours}} {minutes, plural, =1{1 minute} other{{minutes} minutes}}'**
  String directCallHoursMinutes(int hours, int minutes);

  /// Direct chat: line in the chat for a call that ended. {name} is who started it; {mine} is 'yes' when the person using this computer started it (English then says "You"), else 'no'. {length} is like "a few seconds", "25 minutes" or "1 hour 5 minutes".
  ///
  /// In en, this message translates to:
  /// **'{mine, select, yes{You} other{{name}}} started a call that lasted {length}'**
  String directCallLine(String name, String mine, String length);

  /// Direct chat: shown instead of the message box while you appear offline.
  ///
  /// In en, this message translates to:
  /// **'You appear offline. Go online to write.'**
  String get directBlockedOffline;

  /// Direct chat: shown instead of the message box while the friend is offline. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'{name} is offline. You can write when you’re both online.'**
  String directBlockedFriendOffline(String name);

  /// Direct chat: the friend's status in the chat header.
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get directOnline;

  /// Direct chat: the friend's status in the chat header when they are online but have not used their computer for 10 minutes or more.
  ///
  /// In en, this message translates to:
  /// **'Away'**
  String get directAway;

  /// Direct chat: the friend's status in the chat header.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get directOffline;

  /// Direct chat: the friend's status in the chat header; {when} is like "5 min ago" or "yesterday".
  ///
  /// In en, this message translates to:
  /// **'Offline · seen {when}'**
  String directOfflineSeen(String when);

  /// Side column: heading (capitals in English) above the list of direct chats. Short.
  ///
  /// In en, this message translates to:
  /// **'DIRECT MESSAGES'**
  String get directHeading;

  /// Side column: preview of the last message of a direct chat when you wrote it. {text} is the message.
  ///
  /// In en, this message translates to:
  /// **'You: {text}'**
  String directPreviewMine(String text);

  /// Side column: screen-reader label of a direct chat row.
  ///
  /// In en, this message translates to:
  /// **'Open the chat with {name}'**
  String directOpenChat(String name);

  /// Direct chat: screen-reader label of the friend's name in the header; opens their profile card.
  ///
  /// In en, this message translates to:
  /// **'Show {name}’s card'**
  String directShowCard(String name);

  /// Direct chat: status in the header when the two are no longer friends.
  ///
  /// In en, this message translates to:
  /// **'No longer a friend'**
  String get directNoLongerFriend;

  /// Direct chat: tooltip of the disabled Call button.
  ///
  /// In en, this message translates to:
  /// **'{name} is offline: calls ring only while a friend is online'**
  String directCallOffline(String name);

  /// Direct chat: tooltip of the disabled Call button during another call.
  ///
  /// In en, this message translates to:
  /// **'Hang up first to call {name}'**
  String directHangUpFirst(String name);

  /// Direct chat: screen-reader label of the pinned-messages pill in the header.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 pinned message} other{{n} pinned messages}}'**
  String directPinnedCount(int n);

  /// Direct chat: tooltip of the pin pill and title of the pinned messages list.
  ///
  /// In en, this message translates to:
  /// **'Pinned messages'**
  String get directPinnedTitle;

  /// Direct chat: note under "Pinned messages": pins are shared and never disappear; at most {max} pins.
  ///
  /// In en, this message translates to:
  /// **'Pinned for both of you, and they don’t disappear. Up to {max}.'**
  String directPinnedNote(int max);

  /// Direct chat: empty list of pinned messages.
  ///
  /// In en, this message translates to:
  /// **'Nothing pinned yet'**
  String get directNothingPinned;

  /// Direct chat: hint under "Nothing pinned yet".
  ///
  /// In en, this message translates to:
  /// **'Hover over a message and pick the pin to keep it here.'**
  String get directNothingPinnedNote;

  /// Direct chat: when a pinned message was written: {day} is "Today", "Yesterday", a weekday or a date, {time} a clock time.
  ///
  /// In en, this message translates to:
  /// **'{day}, {time}'**
  String directDayTime(String day, String time);

  /// Direct chat: what a message without text holds, in the list of pins: several files (always 2 or more).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, other{{n} files}}'**
  String directFilesCount(int n);

  /// Direct chat: screen-reader label of a pinned message; goes to it in the chat. {name} is who wrote it.
  ///
  /// In en, this message translates to:
  /// **'Show the message from {name}'**
  String directShowMessageFrom(String name);

  /// Direct chat: tooltip and label of the button that unpins a message. Short.
  ///
  /// In en, this message translates to:
  /// **'Unpin'**
  String get directUnpin;

  /// Direct chat: tooltip and label of the button that pins a message (keeps it in the pinned list). Short.
  ///
  /// In en, this message translates to:
  /// **'Pin'**
  String get directPin;

  /// Direct chat: tooltip and label of the button beside a message that deletes it from this computer only. Short.
  ///
  /// In en, this message translates to:
  /// **'Delete for me'**
  String get directDeleteMessage;

  /// Direct chat: title of the question asked before deleting one message from this computer.
  ///
  /// In en, this message translates to:
  /// **'Delete this message?'**
  String get directDeleteMessageTitle;

  /// Direct chat: text of the question before deleting one message: it goes from this computer with its files, and the friend ({name}) keeps their copy.
  ///
  /// In en, this message translates to:
  /// **'It goes from this computer, with its pictures and files. {name} keeps their copy.'**
  String directDeleteMessageNote(String name);

  /// Direct chat: the red button that deletes the message. Short.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get directDeleteMessageAction;

  /// Direct chat: button in the question before deleting one of this person's own messages: deletes it on both devices. Short.
  ///
  /// In en, this message translates to:
  /// **'Delete for both'**
  String get directDeleteForBoth;

  /// Direct chat: text of the question before deleting one of this person's own messages. It names the two buttons ('Delete for both', 'Delete for me'); use their names as translated there. {name} is the friend.
  ///
  /// In en, this message translates to:
  /// **'Delete for both also takes it out of {name}’s chat, as soon as their Fogata gets it. Delete for me leaves it there.'**
  String directDeleteOwnNote(String name);

  /// Direct chat: screen-reader label of the timer pill in the header; {time} is the current choice, like "Off", "1 day" or "1 week".
  ///
  /// In en, this message translates to:
  /// **'Disappearing messages: {time}'**
  String directTimerLabel(String time);

  /// Direct chat: tooltip of the timer pill and title of its menu: messages that delete themselves after a while.
  ///
  /// In en, this message translates to:
  /// **'Disappearing messages'**
  String get directTimerTitle;

  /// Screen-reader label of the area around an open menu; touching it closes the menu.
  ///
  /// In en, this message translates to:
  /// **'Close the menu'**
  String get directCloseMenu;

  /// Direct chat: note in the disappearing messages menu.
  ///
  /// In en, this message translates to:
  /// **'New messages vanish on both devices, with their pictures and files.'**
  String get directTimerNote;

  /// Direct chat: shown under the friend's name in a chat with no messages yet.
  ///
  /// In en, this message translates to:
  /// **'What you write is kept on your two computers only.'**
  String get directEmptyNote;

  /// Direct chat: screen-reader label of a pin line in the chat (like "Marta pinned a message"); a click shows the pinned message. {line} is that line.
  ///
  /// In en, this message translates to:
  /// **'{line}: show it'**
  String directPinLineLabel(String line);

  /// Direct chat: tooltip of a pin line in the chat: shows the pinned message.
  ///
  /// In en, this message translates to:
  /// **'Show the message'**
  String get directShowMessage;

  /// Direct chat: tooltip on a pinned message: it does not disappear.
  ///
  /// In en, this message translates to:
  /// **'Pinned: it stays until unpinned'**
  String get directPinnedStays;

  /// Direct chat: small label beside the time of a pinned message. Short.
  ///
  /// In en, this message translates to:
  /// **'Pinned'**
  String get directPinned;

  /// Direct chat: tooltip of the pin button when the most pins are reached.
  ///
  /// In en, this message translates to:
  /// **'Up to {max} messages can be pinned: unpin one first'**
  String directPinsFull(int max);

  /// Direct chat: under your last message: it reached the friend.
  ///
  /// In en, this message translates to:
  /// **'Delivered'**
  String get directDelivered;

  /// Direct chat: under your last message: it never reached the friend (no longer friends).
  ///
  /// In en, this message translates to:
  /// **'Not delivered'**
  String get directNotDelivered;

  /// Direct chat: under your last message while it is being sent.
  ///
  /// In en, this message translates to:
  /// **'Sending…'**
  String get directSending;

  /// Direct chat: under your last message: it will be sent when both are online. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'Waiting for {name} to come online · sent when you both are'**
  String directWaiting(String name);

  /// Direct chat: about a file not here yet: it can come once you stop appearing offline. Lower case: follows "file name · size · ".
  ///
  /// In en, this message translates to:
  /// **'available when you go online'**
  String get directAvailableWhenYouOnline;

  /// Direct chat: about a file not here yet: it can come when the friend is online. Lower case: follows "file name · size · ". {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'available when {name} is online'**
  String directAvailableWhenOnline(String name);

  /// Direct chat: caption part of a picture that came without being asked (small pictures download automatically). Lower case: follows "name · size · ".
  ///
  /// In en, this message translates to:
  /// **'downloaded automatically'**
  String get directDownloadedByItself;

  /// Direct chat: screen-reader label of a picture; opens it large. {name} is the file name.
  ///
  /// In en, this message translates to:
  /// **'Open {name}'**
  String directOpenFile(String name);

  /// Direct chat: screen-reader label of a video; plays it. {name} is the file name.
  ///
  /// In en, this message translates to:
  /// **'Play {name}'**
  String directPlayFile(String name);

  /// Direct chat: button on a video not fetched yet; {size} is like "48.2 MB".
  ///
  /// In en, this message translates to:
  /// **'Download · {size}'**
  String directDownloadSize(String size);

  /// Direct chat: caption of a video being fetched or ready to fetch; {name} is the file name, {length} its duration like 1:05.
  ///
  /// In en, this message translates to:
  /// **'{name} · {length} · plays once it is here'**
  String directVideoPlaysWhenHere(String name, String length);

  /// Direct chat: caption of a video that cannot be fetched now; {name} is the file name, {length} its duration, {when} says when it can come ("available when Sofía is online").
  ///
  /// In en, this message translates to:
  /// **'{name} · {length} · {when}'**
  String directVideoNotHere(String name, String length, String when);

  /// Direct chat: a file being received, with how much has come in percent.
  ///
  /// In en, this message translates to:
  /// **'Getting it · {percent}%'**
  String directGettingIt(int percent);

  /// Direct chat: under a file that cannot be fetched now: {size} like "48.2 MB", {when} says when it can come ("available when Sofía is online").
  ///
  /// In en, this message translates to:
  /// **'{size} · {when}'**
  String directFileNotHere(String size, String when);

  /// Direct chat: tooltip of the Save button: saves the file in the Downloads folder.
  ///
  /// In en, this message translates to:
  /// **'Save to Downloads'**
  String get directSaveToDownloads;

  /// Direct chat: a file that was deleted from this computer to free disk space; it can be asked again.
  ///
  /// In en, this message translates to:
  /// **'Removed here to free space'**
  String get directRemovedForSpace;

  /// Direct chat: button that fetches a removed file again from the friend. Short.
  ///
  /// In en, this message translates to:
  /// **'Ask again'**
  String get directAskAgain;

  /// Direct chat: screen-reader label of the dark area around a large picture; touching it closes it.
  ///
  /// In en, this message translates to:
  /// **'Close the picture'**
  String get directClosePicture;

  /// Direct chat: screen-reader label of the dark area around a video; touching it closes it.
  ///
  /// In en, this message translates to:
  /// **'Close the video'**
  String get directCloseVideo;

  /// Direct chat: the video player failed.
  ///
  /// In en, this message translates to:
  /// **'This computer cannot play this video.'**
  String get directCannotPlay;

  /// Video player: tooltip of the pause button.
  ///
  /// In en, this message translates to:
  /// **'Pause'**
  String get directPause;

  /// Video player: tooltip of the play button.
  ///
  /// In en, this message translates to:
  /// **'Play'**
  String get directPlay;

  /// Video player: tooltip of the button that turns the sound back on.
  ///
  /// In en, this message translates to:
  /// **'Sound on'**
  String get directSoundOn;

  /// Video player: tooltip of the button that mutes the video's sound.
  ///
  /// In en, this message translates to:
  /// **'Mute'**
  String get directMute;

  /// Direct chat: title of the question before taking out files waiting to be sent (Esc).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{Remove the file?} other{Remove the {n} files?}}'**
  String directRemoveFilesTitle(int n);

  /// Direct chat: text of the question before taking out files waiting to be sent.
  ///
  /// In en, this message translates to:
  /// **'They are taken out of this message. The files on your computer stay as they are.'**
  String get directRemoveFilesText;

  /// Direct chat: note under the message box while writing is not possible.
  ///
  /// In en, this message translates to:
  /// **'What you were writing stays here for when you can send it.'**
  String get directDraftKept;

  /// Direct chat: placeholder and label of the message box: write to {name} (the friend's name).
  ///
  /// In en, this message translates to:
  /// **'Message {name}'**
  String directMessagePlaceholder(String name);

  /// Direct chat: placeholder of the message box while files wait to be sent.
  ///
  /// In en, this message translates to:
  /// **'Add a message (optional)'**
  String get directAddMessageOptional;

  /// Direct chat: note under the message box.
  ///
  /// In en, this message translates to:
  /// **'Pictures up to 10 MB download automatically; videos and files when you open them.'**
  String get directAutoDownloadNote;

  /// Direct chat: tooltip and label of the microphone button beside the message box; one click starts recording a voice message.
  ///
  /// In en, this message translates to:
  /// **'Record a voice message'**
  String get directVoiceRecord;

  /// Direct chat: tooltip of the dimmed microphone button while in a call or a voice room (the call has the microphone).
  ///
  /// In en, this message translates to:
  /// **'You can’t record during a call'**
  String get directVoiceInCall;

  /// Direct chat: toast when the microphone button is clicked while a voice message recorded in the chat with {name} still waits to be sent or deleted; nothing starts recording.
  ///
  /// In en, this message translates to:
  /// **'Send or delete the voice message in your chat with {name} first.'**
  String directVoiceWaitingElsewhere(String name);

  /// Direct chat, recording a voice message: tooltip and label of the trash button that deletes the recording (also Esc).
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get directVoiceDelete;

  /// Direct chat: toast when Send was pressed on a voice message shorter than half a second, which is not sent and is thrown away.
  ///
  /// In en, this message translates to:
  /// **'Too short to send: a voice message takes at least half a second.'**
  String get directVoiceTooShort;

  /// Direct chat, recording a voice message: small pill in the last minute of the 15 allowed, with the time left ({time} like 0:48).
  ///
  /// In en, this message translates to:
  /// **'{time} left'**
  String directVoiceLeft(String time);

  /// Direct chat, recording a voice message: start of the line under the recording bar, naming the microphone chosen in Settings › Voice & video (or 'System default').
  ///
  /// In en, this message translates to:
  /// **'Recording from {microphone}'**
  String directVoiceRecordingFrom(String microphone);

  /// Direct chat, recording a voice message: start of the line under the recording bar when the microphone chosen in Settings › Voice & video could not be opened and the system's default microphone records instead (in place of 'Recording from {microphone}').
  ///
  /// In en, this message translates to:
  /// **'The chosen microphone isn’t available: recording from the default one'**
  String get directVoiceRecordingDefault;

  /// Direct chat, recording a voice message: second part of the line under the recording bar. {esc} and {enter} are the key names Esc and Enter, shown as key caps: Esc deletes the recording, Enter sends it.
  ///
  /// In en, this message translates to:
  /// **'{esc} deletes it, {enter} sends it'**
  String directVoiceKeys(String esc, String enter);

  /// Direct chat, recording a voice message: second part of the line under the recording bar once the recording is longer than 5 seconds, when Esc no longer deletes it (only the Delete button does). {enter} is the key name Enter, shown as a key cap.
  ///
  /// In en, this message translates to:
  /// **'{enter} sends it'**
  String directVoiceKeySend(String enter);

  /// List of direct chats: the last line when it is a voice message, with its length ({length} like 0:18). Mine get 'You: ' before it.
  ///
  /// In en, this message translates to:
  /// **'Voice message · {length}'**
  String directVoicePreview(String length);

  /// Direct chat: what screen readers say for a voice message in the chat.
  ///
  /// In en, this message translates to:
  /// **'Voice message'**
  String get directVoiceMessage;

  /// Direct chat: bold start of the amber note over the message box when the microphone button found no microphone it could use.
  ///
  /// In en, this message translates to:
  /// **'Fogata can’t use the microphone.'**
  String get directVoiceNoMic;

  /// Direct chat, recording a voice message: note over the recording bar when the microphone stopped working part way through. What was recorded until then is kept and can be sent.
  ///
  /// In en, this message translates to:
  /// **'The microphone stopped. You can still send what was recorded.'**
  String get directVoiceMicStopped;

  /// Direct chat: the rest of the amber note when no microphone can be used. 'Voice & video' is the settings section of that name; on macOS it also points to the system's privacy settings.
  ///
  /// In en, this message translates to:
  /// **'{system, select, macos{Check that one is plugged in, chosen in Voice & video and allowed in System Settings › Privacy & Security.} other{Check that one is plugged in and chosen in Voice & video.}}'**
  String directVoiceNoMicCheck(String system);

  /// Direct chat: tooltip and label of the small pill on a voice message that changes how fast it plays (1×, 1.5×, 2×).
  ///
  /// In en, this message translates to:
  /// **'Playback speed'**
  String get directVoiceSpeed;

  /// Direct chat: tooltip and label of the small blue dot on a friend's voice message not played yet.
  ///
  /// In en, this message translates to:
  /// **'Not played yet'**
  String get directVoiceNotPlayed;

  /// Direct chat: tooltip of the button that takes a waiting file out of the message. {name} is the file name.
  ///
  /// In en, this message translates to:
  /// **'Remove {name}'**
  String directRemoveFile(String name);

  /// Direct chat: warning: the chat's stored history was changed or damaged outside Fogata.
  ///
  /// In en, this message translates to:
  /// **'This chat’s history on this computer changed since Fogata last saved it. Some messages may be missing.'**
  String get directAltered;

  /// Direct chat: shown instead of the message box after the friendship ended.
  ///
  /// In en, this message translates to:
  /// **'You and {name} are no longer friends. This chat can only be read.'**
  String directEnded(String name);

  /// Direct chat: danger button that deletes the chat from this computer. Short.
  ///
  /// In en, this message translates to:
  /// **'Delete chat'**
  String get directDeleteChat;

  /// Direct chat: title of the question before deleting a chat.
  ///
  /// In en, this message translates to:
  /// **'Delete the chat with {name}?'**
  String directDeleteTitle(String name);

  /// Direct chat: text of the question before deleting a chat.
  ///
  /// In en, this message translates to:
  /// **'Its messages, pictures, videos and files go from this computer. {name} keeps their copy.'**
  String directDeleteText(String name);

  /// Direct chat: danger button that confirms deleting the chat. Short.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get directDelete;

  /// Direct chat: friend card side: what messages take on disk (a size follows). Short.
  ///
  /// In en, this message translates to:
  /// **'Messages'**
  String get directUsageMessages;

  /// Direct chat: friend card side: what pictures take on disk. Short.
  ///
  /// In en, this message translates to:
  /// **'Pictures'**
  String get directUsagePictures;

  /// Direct chat: friend card side: what videos take on disk. Short.
  ///
  /// In en, this message translates to:
  /// **'Videos'**
  String get directUsageVideos;

  /// Direct chat: friend card side: what other files take on disk. Short.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get directUsageFiles;

  /// Direct chat: heading (capitals in English) of the box with what the chat takes on disk. Short: fits a 270-point-wide box.
  ///
  /// In en, this message translates to:
  /// **'THIS CHAT ON THIS DEVICE'**
  String get directUsageHeading;

  /// Direct chat: button that opens Settings › Storage & downloads.
  ///
  /// In en, this message translates to:
  /// **'Free up space…'**
  String get directFreeUpSpace;

  /// Direct chat: tooltip of the + button beside the message box.
  ///
  /// In en, this message translates to:
  /// **'Add pictures, videos or files'**
  String get directAddFilesTooltip;

  /// Direct chat: screen-reader label of the + button beside the message box.
  ///
  /// In en, this message translates to:
  /// **'Add files'**
  String get directAddFiles;

  /// Account move: how long a move of the account to another computer still takes. Lower case: it follows a percentage, as in "42% · less than a minute left".
  ///
  /// In en, this message translates to:
  /// **'less than a minute left'**
  String get accountMoveLessThanMinuteLeft;

  /// Account move: how long the move still takes. Lower case: it follows a percentage.
  ///
  /// In en, this message translates to:
  /// **'about a minute left'**
  String get accountMoveAboutMinuteLeft;

  /// Account move: how long the move still takes, in minutes (2 or more). Lower case: it follows a percentage.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{about a minute left} other{about {n} minutes left}}'**
  String accountMoveAboutMinutesLeft(int n);

  /// Account move: why it stopped: not enough free disk space on this computer. {need} and {free} are sizes like "1.2 GB".
  ///
  /// In en, this message translates to:
  /// **'This computer needs {need} free for the account, and has {free}.'**
  String accountMoveNoRoom(String need, String free);

  /// Account move: why it stopped: the other computer offered a suspiciously large amount of data. {size} is a size like "2.1 TB".
  ///
  /// In en, this message translates to:
  /// **'The other computer offered {size}, far more than an account holds.'**
  String accountMoveTooLarge(String size);

  /// Account move: why it stopped: another move into this computer is not finished.
  ///
  /// In en, this message translates to:
  /// **'This computer is finishing another move: finish or discard it first.'**
  String get accountMoveBusy;

  /// Account move: why it stopped: asked to finish a move this computer does not know.
  ///
  /// In en, this message translates to:
  /// **'This computer has no move to finish from that computer.'**
  String get accountMoveNothingToFinish;

  /// Account move: why it stopped: this computer's network part stopped working.
  ///
  /// In en, this message translates to:
  /// **'The network stopped.'**
  String get accountMoveNetworkStopped;

  /// Account move: why it stopped: the connection between the two computers broke.
  ///
  /// In en, this message translates to:
  /// **'The connection was lost.'**
  String get accountMoveConnectionLost;

  /// Account move: what the other computer said: it lacks disk space.
  ///
  /// In en, this message translates to:
  /// **'The new computer doesn’t have enough free space.'**
  String get accountMovePeerNoRoom;

  /// Account move: what the other computer said: the account is too large to move.
  ///
  /// In en, this message translates to:
  /// **'The account is too large to move.'**
  String get accountMovePeerTooLarge;

  /// Account move: what the other computer said: it is busy with another move.
  ///
  /// In en, this message translates to:
  /// **'The new computer is finishing another move.'**
  String get accountMovePeerBusy;

  /// Account move: what the other computer said: it could not store what came.
  ///
  /// In en, this message translates to:
  /// **'The new computer could not keep the files.'**
  String get accountMovePeerCouldNotKeep;

  /// Account move: what the other computer said: its person cancelled.
  ///
  /// In en, this message translates to:
  /// **'The move was cancelled on the other computer.'**
  String get accountMovePeerCancelled;

  /// Account move: what the other computer said: it could not finish yet.
  ///
  /// In en, this message translates to:
  /// **'The new computer could not finish the move yet.'**
  String get accountMovePeerCouldNotFinish;

  /// Account move: what the other computer said: the move was stopped there.
  ///
  /// In en, this message translates to:
  /// **'The move was stopped on the other computer.'**
  String get accountMovePeerStopped;

  /// Account move: the other computer knows no move to finish from this computer.
  ///
  /// In en, this message translates to:
  /// **'That computer has no move to finish from this one.'**
  String get accountMovePeerNothingToFinish;

  /// Account move: the other computer failed to prove it received this move.
  ///
  /// In en, this message translates to:
  /// **'That computer did not prove it has this move.'**
  String get accountMovePeerNotProven;

  /// Account move: why it stopped: the code typed did not match.
  ///
  /// In en, this message translates to:
  /// **'The other computer did not accept the code, so nothing was sent.'**
  String get accountMoveWrongCode;

  /// Account move: why it stopped: the code ran out (codes last 10 minutes).
  ///
  /// In en, this message translates to:
  /// **'The code expired before it was used.'**
  String get accountMoveExpired;

  /// Account move: why it stopped: too many wrong codes were tried.
  ///
  /// In en, this message translates to:
  /// **'Several wrong codes were tried; the code stopped working.'**
  String get accountMoveTooManyTries;

  /// Account move: why it stopped: the two computers never connected.
  ///
  /// In en, this message translates to:
  /// **'The two computers could not reach each other.'**
  String get accountMoveNoContact;

  /// Account move: why it stopped: everything went, but the last confirmation did not arrive.
  ///
  /// In en, this message translates to:
  /// **'The very end of the move was not confirmed.'**
  String get accountMoveUnconfirmed;

  /// Account move: the move was cancelled on this computer.
  ///
  /// In en, this message translates to:
  /// **'Cancelled.'**
  String get accountMoveCancelled;

  /// Account move: why it stopped: it broke off midway, and nothing changed on either computer.
  ///
  /// In en, this message translates to:
  /// **'The move stopped before the end; nothing changed.'**
  String get accountMoveInterrupted;

  /// Account move: it broke off midway; {why} is a sentence saying why (it may be technical text from the system, in English).
  ///
  /// In en, this message translates to:
  /// **'The move stopped before the end; nothing changed. {why}'**
  String accountMoveInterruptedWhy(String why);

  /// Account move: the other computer stopped the move.
  ///
  /// In en, this message translates to:
  /// **'The other computer stopped the move.'**
  String get accountMoveDeclined;

  /// Account move: the other computer stopped the move; {why} is a sentence with its reason (it may be the other computer's own words).
  ///
  /// In en, this message translates to:
  /// **'The other computer stopped the move. {why}'**
  String accountMoveDeclinedWhy(String why);

  /// Account move: everything arrived but could not be put in place yet; it will be at the next start.
  ///
  /// In en, this message translates to:
  /// **'The account arrived, and is put in place the next time Fogata starts here.'**
  String get accountMoveLater;

  /// Account move: everything arrived but could not be put in place yet; {why} is a sentence saying why (technical text from the system, in English).
  ///
  /// In en, this message translates to:
  /// **'The account arrived, and is put in place the next time Fogata starts here. {why}'**
  String accountMoveLaterWhy(String why);

  /// Account move: the move ended without finishing, with no reason known.
  ///
  /// In en, this message translates to:
  /// **'It did not finish.'**
  String get accountMoveDidNotFinish;

  /// Account move: this computer's name as the other computer shows it, when the system gives none. Lower case: shown inside quotes in sentences.
  ///
  /// In en, this message translates to:
  /// **'another computer'**
  String get accountMoveAnotherComputer;

  /// Account move: words for the other computer when it gave no name that shows. Lower case: it goes inside sentences, often in quotes ("Move your account to “the other computer”?").
  ///
  /// In en, this message translates to:
  /// **'the other computer'**
  String get accountMoveTheOtherComputer;

  /// Move account: short note shown when the person tries to stop bringing an account here, but it is already being put in place.
  ///
  /// In en, this message translates to:
  /// **'Too late to stop: the account is being put in place.'**
  String get moveTooLateBeingPlaced;

  /// Move account: short note: throwing away the account that came is no longer possible.
  ///
  /// In en, this message translates to:
  /// **'Too late: the account is being put in place.'**
  String get moveTooLateDiscard;

  /// Move account: short note on the old computer: the move can no longer be stopped.
  ///
  /// In en, this message translates to:
  /// **'Too late to stop: the other computer has the account now.'**
  String get moveTooLateOtherHasIt;

  /// Move account: a row in the list of what moves: the number of friends.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 friend} other{{n} friends}}'**
  String moveFriends(int n);

  /// Move account: a row in the list of what moves: the number of groups.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 group} other{{n} groups}}'**
  String moveGroups(int n);

  /// Move account: a row in the list of what moves: the number of groups, with their encryption keys.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 group, with their keys} other{{n} groups, with their keys}}'**
  String moveGroupsWithKeys(int n);

  /// Move account: a row in the list of what moves: the number of direct-message chats.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 chat} other{{n} chats}}'**
  String moveChats(int n);

  /// Move account: right side of the chats row: the total size of everything that moves, like "48.2 MB in all".
  ///
  /// In en, this message translates to:
  /// **'{size} in all'**
  String moveInAll(String size);

  /// Move account: a row in the list of what moves: the app settings.
  ///
  /// In en, this message translates to:
  /// **'Your settings'**
  String get moveYourSettings;

  /// Move account: right side of the settings row: the chosen devices do not move. Lower case, short.
  ///
  /// In en, this message translates to:
  /// **'not the microphone or camera chosen'**
  String get moveNotDevices;

  /// Move account: progress line while the move finishes.
  ///
  /// In en, this message translates to:
  /// **'Finishing on both computers…'**
  String get moveFinishingBoth;

  /// Move account: progress of the move as a percentage (write the percent sign as the language does).
  ///
  /// In en, this message translates to:
  /// **'{percent}%'**
  String movePercent(int percent);

  /// Move account: progress of the move as a percentage, then how long is left ({left} is like "about 3 minutes left").
  ///
  /// In en, this message translates to:
  /// **'{percent}% · {left}'**
  String movePercentLeft(int percent, String left);

  /// Move account: a step of the move in the progress list.
  ///
  /// In en, this message translates to:
  /// **'Identity and profile'**
  String get moveStepIdentity;

  /// Move account: a step of the move in the progress list: the friends.
  ///
  /// In en, this message translates to:
  /// **'Friends'**
  String get moveStepFriends;

  /// Move account: a step of the move in the progress list.
  ///
  /// In en, this message translates to:
  /// **'Groups and their keys'**
  String get moveStepGroups;

  /// Move account: a step of the move in the progress list.
  ///
  /// In en, this message translates to:
  /// **'Chats, pictures and files'**
  String get moveStepChats;

  /// Move account: how much of the chats and files moved so far, sizes like "12 MB of 48.2 MB". Short.
  ///
  /// In en, this message translates to:
  /// **'{done} of {total}'**
  String moveBytesOf(String done, String total);

  /// Move account: last step of the move in the progress list.
  ///
  /// In en, this message translates to:
  /// **'Finishing on both computers'**
  String get moveStepFinishing;

  /// Move account: title on the new computer while the account comes; {name} is the account's name.
  ///
  /// In en, this message translates to:
  /// **'Bringing {name}’s account'**
  String moveBringingNamed(String name);

  /// Move account: title on the new computer while the account comes, when its name is not known yet.
  ///
  /// In en, this message translates to:
  /// **'Bringing your account'**
  String get moveBringingYours;

  /// Move account: note under the progress on the new computer.
  ///
  /// In en, this message translates to:
  /// **'Keep both computers on and Fogata open.'**
  String get moveKeepBothOn;

  /// Move account: note under the code on the new computer.
  ///
  /// In en, this message translates to:
  /// **'Keep Fogata open on both computers. It is quickest on the same network; otherwise they connect over the internet, directly.'**
  String get moveKeepOpenNote;

  /// Move account: title on the new computer when everything came and only the end is missing; {name} is the account's name.
  ///
  /// In en, this message translates to:
  /// **'Finish bringing {name}’s account'**
  String moveFinishBringingNamed(String name);

  /// Move account: title on the new computer when everything came and only the end is missing.
  ///
  /// In en, this message translates to:
  /// **'Finish bringing your account'**
  String get moveFinishBringingYours;

  /// Move account: under the title when the old computer's name is not known.
  ///
  /// In en, this message translates to:
  /// **'Everything arrived. Type this code on the other computer to finish.'**
  String get moveEverythingCame;

  /// Move account: under the title; {computer} is the old computer's name.
  ///
  /// In en, this message translates to:
  /// **'Everything arrived. Type this code on “{computer}” to finish.'**
  String moveEverythingCameFrom(String computer);

  /// Move account: title on the new computer above the code to type on the old one.
  ///
  /// In en, this message translates to:
  /// **'Bring your account here'**
  String get moveBringHereTitle;

  /// Move account: under the title above the code.
  ///
  /// In en, this message translates to:
  /// **'Type this code on the computer that has your account.'**
  String get moveBringHereSub;

  /// Move account: tooltip of the eye button that blurs the code.
  ///
  /// In en, this message translates to:
  /// **'Hide the code'**
  String get moveHideCode;

  /// Move account: tooltip of the eye button that shows the blurred code.
  ///
  /// In en, this message translates to:
  /// **'Show the code'**
  String get moveShowCode;

  /// Move account: under the code: how long it still works; {time} is a countdown like 9:41 (drawn in another font). Lower case.
  ///
  /// In en, this message translates to:
  /// **'expires in {time}'**
  String moveExpiresIn(String time);

  /// Move account: a numbered step; {what} is the bold name of what the other computer shows ("Waiting to confirm").
  ///
  /// In en, this message translates to:
  /// **'On that computer, Fogata shows {what}.'**
  String moveStepFogataShows(String what);

  /// Move account: bold name of what the old computer shows while the move waits; goes inside a step sentence.
  ///
  /// In en, this message translates to:
  /// **'Waiting to confirm'**
  String get moveWaitingToConfirm;

  /// Move account: a numbered step.
  ///
  /// In en, this message translates to:
  /// **'On that computer, open Fogata.'**
  String get moveStepOpenThere;

  /// Move account: a numbered step; {where} is the bold title of a part of the screen on the other computer ("Finish the move", "Move to another computer").
  ///
  /// In en, this message translates to:
  /// **'Type the code under {where}.'**
  String moveStepTypeUnder(String where);

  /// Move account: title of the box where the code is typed to finish a move; also named in a step on the new computer.
  ///
  /// In en, this message translates to:
  /// **'Finish the move'**
  String get moveFinishTheMove;

  /// Move account: a numbered step; {where} is the bold place in the app ("Settings → Move account").
  ///
  /// In en, this message translates to:
  /// **'Go to {where}.'**
  String moveStepGoTo(String where);

  /// Move account: the place in the app where a move starts: the Settings, then its part Move account. Use the same words as the Settings list.
  ///
  /// In en, this message translates to:
  /// **'Settings → Move account'**
  String get moveSettingsPath;

  /// Move account: title of the box in Settings › Move account that sends the account away; also named in a step on the new computer.
  ///
  /// In en, this message translates to:
  /// **'Move to another computer'**
  String get moveToAnotherComputer;

  /// Move account: status pill on the new computer while it waits.
  ///
  /// In en, this message translates to:
  /// **'Waiting for your other computer…'**
  String get moveWaitingForOther;

  /// Move account: warning under the code: someone typed wrong codes against it.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{A wrong code was typed on another computer.} other{Wrong codes were typed {n} times on other computers.}}'**
  String moveWrongCodesTyped(int n);

  /// Move account: button: throw away the account that came instead of finishing the move.
  ///
  /// In en, this message translates to:
  /// **'Throw it away instead…'**
  String get moveThrowAwayInstead;

  /// Move account: title on the new computer once the code matched.
  ///
  /// In en, this message translates to:
  /// **'Found your other computer'**
  String get moveFoundOther;

  /// Move account: under the title: the computers are on the same local network.
  ///
  /// In en, this message translates to:
  /// **'Same network · encrypted between the two'**
  String get moveFoundSameNetwork;

  /// Move account: under the title: the computers connect over the internet.
  ///
  /// In en, this message translates to:
  /// **'Over the internet · encrypted between the two'**
  String get moveFoundInternet;

  /// Move account: status on either computer once its person said yes: waiting for a yes on the other one.
  ///
  /// In en, this message translates to:
  /// **'Confirm on the other computer to start'**
  String get moveConfirmThere;

  /// Move account: note on the new computer when the account offered is the one this computer already runs (instead of a warning that its profile is replaced).
  ///
  /// In en, this message translates to:
  /// **'This is the account already on this computer. Nothing is lost by bringing it here.'**
  String get moveSameAccountNote;

  /// Move account: button on the new computer once the code matched: take the account shown, which replaces what this computer has.
  ///
  /// In en, this message translates to:
  /// **'Bring it here'**
  String get moveTakeYes;

  /// Move account: title once the account is on this computer.
  ///
  /// In en, this message translates to:
  /// **'Welcome back'**
  String get moveWelcomeBack;

  /// Move account: title once the account is on this computer; {name} is the account's name.
  ///
  /// In en, this message translates to:
  /// **'Welcome back, {name}'**
  String moveWelcomeBackNamed(String name);

  /// Move account: under the title once the move is done.
  ///
  /// In en, this message translates to:
  /// **'Your account is on this computer now. The other computer stopped using it.'**
  String get moveDoneSub;

  /// Move account: button once the move is done: open the app with the account.
  ///
  /// In en, this message translates to:
  /// **'Open Fogata'**
  String get moveOpenFogata;

  /// Move account: small label under a crossed-out code that ran out. Lower case.
  ///
  /// In en, this message translates to:
  /// **'expired'**
  String get moveCodeExpiredLabel;

  /// Move account: small label under a crossed-out code that stopped working. Lower case.
  ///
  /// In en, this message translates to:
  /// **'stopped working'**
  String get moveCodeStoppedLabel;

  /// Move account: note after a code ran out.
  ///
  /// In en, this message translates to:
  /// **'This code expired before your other computer used it. Codes last 10 minutes.'**
  String get moveCodeExpiredNote;

  /// Move account: note after a code stopped working.
  ///
  /// In en, this message translates to:
  /// **'Wrong codes were typed several times on other computers, so this code stopped working.'**
  String get moveCodeStoppedNote;

  /// Move account: button: show a new code.
  ///
  /// In en, this message translates to:
  /// **'New code'**
  String get moveNewCode;

  /// Move account: title when everything came but the move is not fully finished.
  ///
  /// In en, this message translates to:
  /// **'Almost there'**
  String get moveAlmostThere;

  /// Move account: under the title when the account will be put in place at the next start.
  ///
  /// In en, this message translates to:
  /// **'Your account arrived whole, but it could not be put in place yet. It will be the next time Fogata starts here.'**
  String get moveLaterSub;

  /// Move account: button: restart the app.
  ///
  /// In en, this message translates to:
  /// **'Start Fogata again'**
  String get moveStartAgain;

  /// Move account: under the title when the end of the move was not confirmed.
  ///
  /// In en, this message translates to:
  /// **'Everything arrived, but the other computer did not confirm the end. Show a code and type it there to finish the move.'**
  String get moveUnconfirmedSub;

  /// Move account: title on the new computer when the old one stopped the move.
  ///
  /// In en, this message translates to:
  /// **'The move was stopped on the other computer'**
  String get moveStoppedThereTitle;

  /// Move account: under the title when the old computer stopped the move.
  ///
  /// In en, this message translates to:
  /// **'Nothing changed: your account is still on the other computer, and none of it was kept here.'**
  String get moveStoppedThereSub;

  /// Move account: title on the new computer when the move failed on this computer.
  ///
  /// In en, this message translates to:
  /// **'The move could not start'**
  String get moveCouldNotStartTitle;

  /// Move account: under the title when nothing more is known.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong on this computer.'**
  String get moveSomethingWrongHere;

  /// Move account: title on the new computer when the connection broke.
  ///
  /// In en, this message translates to:
  /// **'The move stopped halfway'**
  String get moveHalfwayTitle;

  /// Move account: under the title when the connection broke.
  ///
  /// In en, this message translates to:
  /// **'The connection to your other computer was lost. Nothing changed: your account is still on the other computer, and none of it was kept here.'**
  String get moveHalfwaySub;

  /// Move account: button: show a new code to finish the move.
  ///
  /// In en, this message translates to:
  /// **'Show a code to finish'**
  String get moveShowCodeToFinish;

  /// Move account: button: start again with a new code.
  ///
  /// In en, this message translates to:
  /// **'Try again with a new code'**
  String get moveTryNewCode;

  /// Move account: button: go back to this computer's own profile; {name} is its name.
  ///
  /// In en, this message translates to:
  /// **'Back to {name}'**
  String moveBackToNamed(String name);

  /// Move account: button: go back to this computer's own profile.
  ///
  /// In en, this message translates to:
  /// **'Back to this profile'**
  String get moveBackToThisProfile;

  /// Move account: button: stop bringing an account and make a new profile.
  ///
  /// In en, this message translates to:
  /// **'Make a new profile instead'**
  String get moveNewProfileInstead;

  /// Move account: title of the dialog asking before throwing away an account that came.
  ///
  /// In en, this message translates to:
  /// **'Throw away the account that arrived?'**
  String get moveDiscardTitle;

  /// Move account: text of that dialog.
  ///
  /// In en, this message translates to:
  /// **'Do this only if the other computer keeps using it. That computer waits to confirm until its person chooses to keep using it there.'**
  String get moveDiscardText;

  /// Move account: button in that dialog: keep the account that came.
  ///
  /// In en, this message translates to:
  /// **'Keep it'**
  String get moveKeepIt;

  /// Move account: red button in that dialog: throw the account away.
  ///
  /// In en, this message translates to:
  /// **'Throw it away'**
  String get moveThrowAway;

  /// Move account: link under the welcome card, and the name of that choice in a step on the old computer.
  ///
  /// In en, this message translates to:
  /// **'Bring your account from another computer'**
  String get moveBringFromAnother;

  /// Move account: first words of the link under the welcome card, before "Bring your account from another computer".
  ///
  /// In en, this message translates to:
  /// **'Already on Fogata?'**
  String get moveAlreadyOnFogata;

  /// Move account: error under the code field: this code was already tried.
  ///
  /// In en, this message translates to:
  /// **'That code already failed to match once. Get a new code on the new computer.'**
  String get moveCodeTriedBefore;

  /// Move account: title on the old computer while the account goes; {computer} is the new computer's name.
  ///
  /// In en, this message translates to:
  /// **'Moving your account to “{computer}”'**
  String moveMovingTo(String computer);

  /// Move account: note under the progress on the old computer.
  ///
  /// In en, this message translates to:
  /// **'Keep both computers on and Fogata open. Fogata is offline here meanwhile.'**
  String get moveKeepBothOnOffline;

  /// Move account: label of the code field (read by screen readers).
  ///
  /// In en, this message translates to:
  /// **'The code shown on the new computer'**
  String get moveCodeFieldLabel;

  /// Move account: explains the move under its title.
  ///
  /// In en, this message translates to:
  /// **'Your identity, friends, groups, chats, files and settings go to the other computer, straight from this one and encrypted. Then this computer stops using your account, so there is never a second you.'**
  String get moveToAnotherSub;

  /// Move account: a numbered step; {choice} is the bold name of the link "Bring your account from another computer".
  ///
  /// In en, this message translates to:
  /// **'On the new computer, open Fogata and choose {choice}.'**
  String moveStepChoose(String choice);

  /// Move account: a numbered step, followed by the code field.
  ///
  /// In en, this message translates to:
  /// **'Type the code it shows:'**
  String get moveStepTypeItShows;

  /// Move account: status pill on the old computer: the person has to say yes or no.
  ///
  /// In en, this message translates to:
  /// **'Waiting for your answer…'**
  String get moveWaitingForAnswer;

  /// Move account: status pill on the old computer while it looks for the new one.
  ///
  /// In en, this message translates to:
  /// **'Looking for the computer showing this code…'**
  String get moveLookingForCode;

  /// Move account: note under the code field.
  ///
  /// In en, this message translates to:
  /// **'Fogata goes offline on this computer while it moves.'**
  String get moveOfflineWhileMoving;

  /// Move account: error after a wrong code.
  ///
  /// In en, this message translates to:
  /// **'The new computer didn’t accept that code, so nothing was sent. Check it and type it again. If it still fails, get a new code there.'**
  String get moveWrongCodeNote;

  /// Move account: bold first sentence of the note when the computers never connected.
  ///
  /// In en, this message translates to:
  /// **'Can’t reach the new computer.'**
  String get moveCantReachNew;

  /// Move account: rest of that note.
  ///
  /// In en, this message translates to:
  /// **'Both looked for each other for 3 minutes. Put them on the same network (the same Wi-Fi, or a cable) and try again. Your account is still here and Fogata is back online.'**
  String get moveCantReachNewNote;

  /// Move account: note on the old computer when the new one stopped the move.
  ///
  /// In en, this message translates to:
  /// **'The move was stopped on the new computer. Nothing was moved: your account is still here and Fogata is back online.'**
  String get moveDeclinedNote;

  /// Move account: note on the old computer when nothing more is known.
  ///
  /// In en, this message translates to:
  /// **'The move could not start.'**
  String get moveCouldNotStart;

  /// Move account: note on the old computer when the connection broke.
  ///
  /// In en, this message translates to:
  /// **'The move stopped halfway: the connection to the new computer was lost. Nothing changed: your account is still here and Fogata is back online.'**
  String get moveHalfwayNote;

  /// Move account: words for this computer's profile when it has no name; goes inside sentences ("Replaces this profile on this computer."). Lower case.
  ///
  /// In en, this message translates to:
  /// **'this profile'**
  String get moveThisProfileWords;

  /// Move account: title of the box in Settings › Move account that brings another account here.
  ///
  /// In en, this message translates to:
  /// **'Bring an account here'**
  String get moveBringCardTitle;

  /// Move account: under that title.
  ///
  /// In en, this message translates to:
  /// **'Show a code on this computer and type it on the one that has the account.'**
  String get moveBringCardSub;

  /// Move account: button that starts bringing an account here.
  ///
  /// In en, this message translates to:
  /// **'Bring an account here…'**
  String get moveBringCardButton;

  /// Move account: beside that button; {name} is this computer's profile.
  ///
  /// In en, this message translates to:
  /// **'Replaces {name} on this computer.'**
  String moveReplacesHere(String name);

  /// Move account: warning before replacing this computer's profile. {bold} is the profile's name drawn in bold (put it where the name first appears); {name} is the same name again.
  ///
  /// In en, this message translates to:
  /// **'{bold}, the profile on this computer now, will be replaced and its {friends, plural, =1{1 friend} other{{friends} friends}}, {groups, plural, =1{1 group} other{{groups} groups}} and {chats, plural, =1{1 chat} other{{chats} chats}} deleted here. To keep {name}, move it to another computer first.'**
  String moveReplaceWarning(
    String bold,
    String name,
    int friends,
    int groups,
    int chats,
  );

  /// Move account: red button: replace this computer's profile {name} and show a code.
  ///
  /// In en, this message translates to:
  /// **'Replace {name} and show a code'**
  String moveReplaceAndShow(String name);

  /// Move account: title of the question on the old computer; {computer} is the new computer's name.
  ///
  /// In en, this message translates to:
  /// **'Move your account to “{computer}”?'**
  String moveConfirmTitle(String computer);

  /// Move account: under that title: the computers share a local network.
  ///
  /// In en, this message translates to:
  /// **'Found on the same network. Everything goes encrypted, straight to it.'**
  String get moveConfirmSameNetwork;

  /// Move account: under that title: the computers connect over the internet.
  ///
  /// In en, this message translates to:
  /// **'Found over the internet. Everything goes encrypted, straight to it.'**
  String get moveConfirmInternet;

  /// Move account: under that title: the new computer's own profile {name} will be replaced.
  ///
  /// In en, this message translates to:
  /// **'It replaces the profile {name} there.'**
  String moveConfirmReplaces(String name);

  /// Move account: warning in the question on the old computer.
  ///
  /// In en, this message translates to:
  /// **'When it is done, this computer stops using your account and can’t use it again. You can then delete what is left here.'**
  String get moveConfirmNote;

  /// Move account: button in that question: start the move.
  ///
  /// In en, this message translates to:
  /// **'Move my account'**
  String get moveConfirmYes;

  /// Move account: title of the dialog on the old computer asking before using the account here again.
  ///
  /// In en, this message translates to:
  /// **'Keep using your account here?'**
  String get moveKeepTitle;

  /// Move account: text of that dialog.
  ///
  /// In en, this message translates to:
  /// **'Only if the new computer does not have it: open Fogata there first. If both used it, your friends and groups would see two of you, and messages would reach only one.'**
  String get moveKeepText;

  /// Move account: red button in that dialog.
  ///
  /// In en, this message translates to:
  /// **'Keep using it here'**
  String get moveKeepYes;

  /// Move account: title on the old computer when the new one has not confirmed the end; {computer} is its name.
  ///
  /// In en, this message translates to:
  /// **'Waiting for “{computer}” to confirm'**
  String moveWaitingFor(String computer);

  /// Move account: under that title.
  ///
  /// In en, this message translates to:
  /// **'Your account was sent, but the connection broke before the new computer said it is ready. This computer won’t use the account meanwhile.'**
  String get moveWaitingText;

  /// Move account: in the box to finish the move.
  ///
  /// In en, this message translates to:
  /// **'Open Fogata on the new computer: it shows a code. Type it here.'**
  String get moveFinishText;

  /// Move account: button: finish the move with the code typed.
  ///
  /// In en, this message translates to:
  /// **'Finish'**
  String get moveFinish;

  /// Move account: error when finishing with a wrong code.
  ///
  /// In en, this message translates to:
  /// **'The new computer didn’t accept that code. Check it and type it again.'**
  String get moveFinishWrongCode;

  /// Move account: error when finishing and the computers never connected.
  ///
  /// In en, this message translates to:
  /// **'Can’t reach the new computer. Put both on the same network and try again.'**
  String get moveFinishNoContact;

  /// Move account: button on the old computer: use the account here again.
  ///
  /// In en, this message translates to:
  /// **'The new computer doesn’t have it: keep using this one…'**
  String get moveKeepThisOne;

  /// Move account: title on the old computer after the move; {computer} is the new computer's name.
  ///
  /// In en, this message translates to:
  /// **'Your account moved to “{computer}”'**
  String moveMovedYours(String computer);

  /// Move account: title on the old computer after the move; {name} is the account's name, {computer} the new computer's.
  ///
  /// In en, this message translates to:
  /// **'{name}’s account moved to “{computer}”'**
  String moveMovedNamed(String name, String computer);

  /// Move account: under that title.
  ///
  /// In en, this message translates to:
  /// **'This computer doesn’t use it any more: friends and groups reach you on the other one.'**
  String get moveMovedNoteYou;

  /// Move account: under that title; {name} is the account's name.
  ///
  /// In en, this message translates to:
  /// **'This computer doesn’t use it any more: friends and groups reach {name} on the other one.'**
  String moveMovedNoteNamed(String name);

  /// Move account: when the account moved, today; {time} is a clock time. A sentence before the note. {hour} is "one" when the clock time's hour is 1 (1:05, 01:05, 1:05 PM), else "other", for languages with an article before the time (Spanish "a la 1:05" but "a las 23:05", Portuguese "à 1:05" but "às 23:05", Italian "all’1:05"). English says the same in both cases.
  ///
  /// In en, this message translates to:
  /// **'{hour, select, one{Today at {time}.} other{Today at {time}.}}'**
  String moveMovedTodayAt(String time, String hour);

  /// Move account: when the account moved, another day; {when} is like "Yesterday 23:14" or "Oct 21, 23:14". A sentence before the note.
  ///
  /// In en, this message translates to:
  /// **'{when}.'**
  String moveMovedOn(String when);

  /// Move account: two sentences in a row: when the account moved, then the note. Change only the space between them if the language writes sentences without one.
  ///
  /// In en, this message translates to:
  /// **'{when} {note}'**
  String moveMovedWhenAndNote(String when, String note);

  /// Move account: title of the box about what stays on the old computer.
  ///
  /// In en, this message translates to:
  /// **'What is left here'**
  String get moveLeftHere;

  /// Move account: text of that box.
  ///
  /// In en, this message translates to:
  /// **'Chats, pictures and files from before the move, encrypted. The other computer has its own copy.'**
  String get moveLeftHereText;

  /// Move account: red button: delete what is left on the old computer.
  ///
  /// In en, this message translates to:
  /// **'Delete Fogata’s data on this computer'**
  String get moveDeleteData;

  /// Move account: note under that box.
  ///
  /// In en, this message translates to:
  /// **'After deleting, Fogata here starts like new.'**
  String get moveAfterDeleting;

  /// Move account: error while typing a code: it is not shaped like one. Keep XXXXX-XXXXX.
  ///
  /// In en, this message translates to:
  /// **'A code has ten letters and digits, like XXXXX-XXXXX.'**
  String get moveCodeNotACode;

  /// Move account: error while typing a code: a letter or digit is wrong.
  ///
  /// In en, this message translates to:
  /// **'That code isn’t right: check the letters and digits.'**
  String get moveCodeMistyped;

  /// Welcome page (first start, before a profile exists): the large headline. It never wraps: the line break (\n) splits it in two balanced lines (place it where it reads best), and the whole headline is drawn smaller when a line is too long for its box.
  ///
  /// In en, this message translates to:
  /// **'Talk like you’re\nin the same room.'**
  String get welcomeHeadline;

  /// Welcome page (first start, before a profile exists): text under the headline.
  ///
  /// In en, this message translates to:
  /// **'Voice, video and chat with friends, family or your team, straight from your computer to theirs.'**
  String get welcomeIntro;

  /// Welcome page (first start, before a profile exists): small chip: no sign-up or account is needed. Very short.
  ///
  /// In en, this message translates to:
  /// **'No account'**
  String get welcomeNoAccount;

  /// Welcome page (first start, before a profile exists): small chip: everything is encrypted. Very short.
  ///
  /// In en, this message translates to:
  /// **'Encrypted'**
  String get welcomeEncrypted;

  /// Welcome page (first start, before a profile exists): small chip: voice rooms exist only while people are in them. Very short.
  ///
  /// In en, this message translates to:
  /// **'Ephemeral rooms'**
  String get welcomeEphemeral;

  /// Welcome page (first start, before a profile exists): description of the welcome picture for screen readers.
  ///
  /// In en, this message translates to:
  /// **'Fogata’s character, a little flame with headphones, making the peace sign'**
  String get welcomeCharacter;

  /// Welcome page (first start, before a profile exists): error when the chosen picture file cannot be read.
  ///
  /// In en, this message translates to:
  /// **'Could not read that image.'**
  String get welcomeImageUnreadable;

  /// Welcome page (first start): title of the dialog shown when the system keychain could not keep the key that encrypts this computer's data.
  ///
  /// In en, this message translates to:
  /// **'Your data can’t be encrypted yet'**
  String get welcomeKeychainTitle;

  /// Welcome page: text of that dialog. {system} is linux (then it explains the Linux keychain: Secret Service, KWallet and GNOME Keyring are product names) or other. Keep the space at the start of each choice if the language separates sentences with spaces.
  ///
  /// In en, this message translates to:
  /// **'Fogata encrypts your identity, friends and direct messages with a key it keeps in the system keychain, and the keychain did not keep it.{system, select, linux{ On Linux the keychain is a running Secret Service, such as KWallet or GNOME Keyring: start or unlock one, then try again.} other{ Unlock or repair the keychain, then try again.}}'**
  String welcomeKeychainText(String system);

  /// Welcome page: button of that dialog that asks the keychain again.
  ///
  /// In en, this message translates to:
  /// **'Try again'**
  String get welcomeKeychainRetry;

  /// Welcome page: button of that dialog that keeps the data unencrypted, knowingly.
  ///
  /// In en, this message translates to:
  /// **'Continue unencrypted'**
  String get welcomeKeychainPlain;

  /// Welcome page: note in that dialog about continuing unencrypted.
  ///
  /// In en, this message translates to:
  /// **'Unencrypted, anyone who copies this computer’s files can read them. Settings will keep saying so, and Fogata encrypts everything the first time it opens with a keychain.'**
  String get welcomeKeychainPlainNote;

  /// Welcome page (first start, before a profile exists): error when Create is pressed with no name.
  ///
  /// In en, this message translates to:
  /// **'Write your name.'**
  String get welcomeWriteName;

  /// Welcome page (first start, before a profile exists): title of the profile card: choose a name and picture.
  ///
  /// In en, this message translates to:
  /// **'How should your people see you?'**
  String get welcomeCardTitle;

  /// Welcome page (first start, before a profile exists): under that title.
  ///
  /// In en, this message translates to:
  /// **'A name and a picture. Change them whenever you like.'**
  String get welcomeCardSub;

  /// Welcome page (first start, before a profile exists): note under the card. Keep "IPFS".
  ///
  /// In en, this message translates to:
  /// **'Nothing goes to a Fogata server: there is none. The public IPFS network is used only as a phone book to find your friends.'**
  String get welcomeNoServer;

  /// Welcome page (first start, before a profile exists): placeholder in the empty name field and in the preview above it.
  ///
  /// In en, this message translates to:
  /// **'Your name'**
  String get welcomeYourName;

  /// Welcome page (first start, before a profile exists): screen-reader label of a ready-made picture; {name} is the picture's name (Duckly, Jiri…).
  ///
  /// In en, this message translates to:
  /// **'Picture: {name}'**
  String welcomePictureLabel(String name);

  /// Welcome page (first start, before a profile exists): tooltip of the button that picks a picture from the computer.
  ///
  /// In en, this message translates to:
  /// **'Upload a picture'**
  String get welcomeUploadPicture;

  /// Welcome page (first start, before a profile exists): screen-reader label of the name field.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get welcomeNameLabel;

  /// Welcome page (first start, before a profile exists): button that makes the profile. Short: it shares a pill with the name field.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get welcomeCreate;

  /// Welcome page (first start, before a profile exists): the Create button for a moment after the profile is made. Short.
  ///
  /// In en, this message translates to:
  /// **'Ready'**
  String get welcomeReady;

  /// Welcome page (first start, before a profile exists): screen-reader label of the Create button.
  ///
  /// In en, this message translates to:
  /// **'Create identity'**
  String get welcomeCreateLabel;

  /// Welcome page (first start, before a profile exists): screen-reader label of the key fingerprint; {key} is the fingerprint.
  ///
  /// In en, this message translates to:
  /// **'Key made on this computer: {key}'**
  String welcomeKeyLabel(String key);

  /// Welcome page (first start, before a profile exists): tooltip of the button that makes a new key.
  ///
  /// In en, this message translates to:
  /// **'Make another key'**
  String get welcomeNewKey;

  /// Red banner at the top when the app's network part stopped working: bold first sentence.
  ///
  /// In en, this message translates to:
  /// **'Fogata stopped working.'**
  String get stoppedTitle;

  /// Red banner at the top when the app's network part stopped working: second sentence while it restarts.
  ///
  /// In en, this message translates to:
  /// **'Starting it again…'**
  String get stoppedStartingAgain;

  /// Red banner at the top when the app's network part stopped working: second sentence: what to do.
  ///
  /// In en, this message translates to:
  /// **'Restart it to connect again.'**
  String get stoppedRestartToConnect;

  /// Red banner at the top when the app's network part stopped working: button. Short.
  ///
  /// In en, this message translates to:
  /// **'Restart now'**
  String get stoppedRestartNow;

  /// Settings: name of the part with the person's picture, name and bio (in the settings list and as the part's title). Short.
  ///
  /// In en, this message translates to:
  /// **'My profile'**
  String get settingsSectionProfile;

  /// Settings: name of the part that moves the account (identity, friends, groups, chats) to another computer. Short.
  ///
  /// In en, this message translates to:
  /// **'Move account'**
  String get settingsSectionMove;

  /// Settings: name of the part with the colour theme. Short.
  ///
  /// In en, this message translates to:
  /// **'Appearance'**
  String get settingsSectionAppearance;

  /// Settings: name of the part with the language and the clock format. Short.
  ///
  /// In en, this message translates to:
  /// **'Language & time'**
  String get settingsSectionLanguageTime;

  /// Settings: name of the part with microphone, speaker and camera. Short.
  ///
  /// In en, this message translates to:
  /// **'Voice & video'**
  String get settingsSectionVoice;

  /// Settings: name of the part with the app's sound effects. Short.
  ///
  /// In en, this message translates to:
  /// **'Sounds'**
  String get settingsSectionSounds;

  /// Settings: name of the part with opening at startup and staying in the tray. Short.
  ///
  /// In en, this message translates to:
  /// **'Behaviour'**
  String get settingsSectionBehaviour;

  /// Settings: name of the part with relaying for group members and the network check. Short.
  ///
  /// In en, this message translates to:
  /// **'Network'**
  String get settingsSectionNetwork;

  /// Settings: name of the part with disk use of direct messages and the downloads folder. Short.
  ///
  /// In en, this message translates to:
  /// **'Storage & downloads'**
  String get settingsSectionStorage;

  /// Settings list: small heading (shown in capitals) above the person's own parts (My profile, Move account).
  ///
  /// In en, this message translates to:
  /// **'User'**
  String get settingsHeadingUser;

  /// Settings list: small heading (shown in capitals) above the app's parts (Appearance, Sounds, Network…).
  ///
  /// In en, this message translates to:
  /// **'App'**
  String get settingsHeadingApp;

  /// Settings: the title at the top of the settings list.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settingsTitle;

  /// Settings, narrow window: tooltip of the back button that returns to the list of settings parts.
  ///
  /// In en, this message translates to:
  /// **'All settings'**
  String get settingsAll;

  /// Settings: tooltip of the close (X) button.
  ///
  /// In en, this message translates to:
  /// **'Close settings'**
  String get settingsCloseTooltip;

  /// Settings › My profile: title of the question when closing the settings with profile changes not saved yet.
  ///
  /// In en, this message translates to:
  /// **'Save your changes?'**
  String get settingsUnsavedSaveTitle;

  /// Settings › My profile: title of the question when closing with profile changes that cannot be saved (a name that breaks the rules, say).
  ///
  /// In en, this message translates to:
  /// **'Discard your changes?'**
  String get settingsUnsavedDiscardTitle;

  /// Settings › My profile: text under "Save your changes?".
  ///
  /// In en, this message translates to:
  /// **'Your profile has changes that are not saved yet.'**
  String get settingsUnsavedSaveText;

  /// Settings › My profile: text under "Discard your changes?".
  ///
  /// In en, this message translates to:
  /// **'Your profile has changes that cannot be saved as they are.'**
  String get settingsUnsavedDiscardText;

  /// Toast after saving the profile (name or picture).
  ///
  /// In en, this message translates to:
  /// **'Profile saved. Friends and groups online see it now, the rest when they connect.'**
  String get settingsProfileSaved;

  /// Toast after saving only the bio.
  ///
  /// In en, this message translates to:
  /// **'Bio saved. Friends online see it now, the rest when they connect.'**
  String get settingsBioSaved;

  /// Screen-reader label of a settings switch that is on. {what} is the setting's name, like "Sounds" or "Keep running".
  ///
  /// In en, this message translates to:
  /// **'{what} on'**
  String settingsSwitchOn(String what);

  /// Screen-reader label of a settings switch that is off. {what} is the setting's name, like "Sounds" or "Keep running".
  ///
  /// In en, this message translates to:
  /// **'{what} off'**
  String settingsSwitchOff(String what);

  /// Settings: word beside a switch that is on. Very short.
  ///
  /// In en, this message translates to:
  /// **'On'**
  String get settingsOn;

  /// Settings: word beside a switch that is off. Very short.
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get settingsOff;

  /// Settings › My profile: title of the block with picture, name and bio.
  ///
  /// In en, this message translates to:
  /// **'Your identity'**
  String get settingsIdentity;

  /// Settings › My profile: note under "Your identity" ("signed": cryptographically, so others know it is really yours).
  ///
  /// In en, this message translates to:
  /// **'Your name and picture travel signed to your friends and groups, your bio to your friends.'**
  String get settingsIdentityNote;

  /// Settings › My profile: label above the profile picture choices; also a button that picks a banner picture.
  ///
  /// In en, this message translates to:
  /// **'Picture'**
  String get settingsPicture;

  /// Settings › My profile: tooltip of the + button that picks a profile picture or animated GIF from the computer.
  ///
  /// In en, this message translates to:
  /// **'Upload a picture or a GIF'**
  String get settingsUploadPicture;

  /// Settings › My profile: label of the name field.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get settingsName;

  /// Settings › My profile: label of the bio field (a few words about the person, shown on their card).
  ///
  /// In en, this message translates to:
  /// **'Bio'**
  String get settingsBio;

  /// Settings › My profile: placeholder in the empty bio field.
  ///
  /// In en, this message translates to:
  /// **'A few words about you'**
  String get settingsBioHint;

  /// Settings › My profile: note under an empty bio field.
  ///
  /// In en, this message translates to:
  /// **'Your card shows no bio.'**
  String get settingsNoBio;

  /// Settings › My profile: note under the bio field; {lines} is the most lines a bio can have (4).
  ///
  /// In en, this message translates to:
  /// **'Your friends see it on your card, in up to {lines} lines.'**
  String settingsBioWhere(int lines);

  /// Settings › My profile: title of the choice of colour or picture at the top of the person's card.
  ///
  /// In en, this message translates to:
  /// **'Panel background'**
  String get settingsBannerTitle;

  /// Settings › My profile: note under "Panel background".
  ///
  /// In en, this message translates to:
  /// **'The top of your card, for you and your friends.'**
  String get settingsBannerNote;

  /// Settings › My profile: button that picks a different banner picture when one is already set. Short.
  ///
  /// In en, this message translates to:
  /// **'Another picture'**
  String get settingsBannerAnother;

  /// Settings › My profile: tooltip of the button that picks a banner picture.
  ///
  /// In en, this message translates to:
  /// **'Use a picture from this computer'**
  String get settingsBannerUpload;

  /// Settings › My profile: label above a code that identifies this computer's key (to compare it with a friend).
  ///
  /// In en, this message translates to:
  /// **'This computer’s fingerprint'**
  String get settingsFingerprint;

  /// Settings › Appearance: title of the colour theme choice.
  ///
  /// In en, this message translates to:
  /// **'Theme'**
  String get settingsTheme;

  /// Settings › Appearance: note under "Theme". "System", "Dark" and "Light" are theme names: use the same words as the theme names.
  ///
  /// In en, this message translates to:
  /// **'The window’s colours. System follows the computer between Dark and Light.'**
  String get settingsThemeNote;

  /// Settings › Voice & video: title of the audio devices block.
  ///
  /// In en, this message translates to:
  /// **'Microphone and speaker'**
  String get settingsMicSpeaker;

  /// Settings › Voice & video: note under "Microphone and speaker".
  ///
  /// In en, this message translates to:
  /// **'What calls hear you with and play through. They can also be changed in the middle of a call.'**
  String get settingsMicSpeakerNote;

  /// Settings › Voice & video: title of the camera block.
  ///
  /// In en, this message translates to:
  /// **'Camera'**
  String get settingsCamera;

  /// Settings › Voice & video: note under "Camera".
  ///
  /// In en, this message translates to:
  /// **'What calls show of you when your camera is on. Try it here before a call.'**
  String get settingsCameraNote;

  /// Settings › Sounds: title of the block.
  ///
  /// In en, this message translates to:
  /// **'Sound effects'**
  String get settingsSoundEffects;

  /// Settings › Sounds: what the sound effects play for.
  ///
  /// In en, this message translates to:
  /// **'Entering and leaving a room, people coming and going, chat messages, the microphone and sharing.'**
  String get settingsSoundEffectsNote;

  /// Settings › Sounds: name of the switch that turns the sound effects on or off (read by screen readers).
  ///
  /// In en, this message translates to:
  /// **'Sounds'**
  String get settingsSounds;

  /// Toast when a system setting (opening at startup) could not be changed; {reason} is the system's own text, often English.
  ///
  /// In en, this message translates to:
  /// **'Fogata could not change that: {reason}'**
  String settingsChangeFailed(String reason);

  /// Settings › Behaviour, macOS: Open at startup waits for approval. Use the names macOS shows in this language for System Settings › General › Login Items.
  ///
  /// In en, this message translates to:
  /// **'Allow Fogata in System Settings › General › Login Items.'**
  String get settingsAutostartApprove;

  /// Settings › Behaviour, macOS: opening at startup needs a newer macOS.
  ///
  /// In en, this message translates to:
  /// **'This needs macOS 13 or later.'**
  String get settingsAutostartOldMac;

  /// Settings › Behaviour: this build cannot open at startup.
  ///
  /// In en, this message translates to:
  /// **'This copy of Fogata cannot start automatically.'**
  String get settingsAutostartUnsupported;

  /// Settings › Behaviour: whether Fogata opens at startup could not be read.
  ///
  /// In en, this message translates to:
  /// **'Fogata could not read this setting from your system.'**
  String get settingsAutostartUnread;

  /// Settings › Behaviour: title and switch name for opening Fogata when the computer starts.
  ///
  /// In en, this message translates to:
  /// **'Open at startup'**
  String get settingsAutostart;

  /// Settings › Behaviour: note under "Open at startup" ("sign in" to the computer's user account).
  ///
  /// In en, this message translates to:
  /// **'Open Fogata when your computer starts, once you sign in.'**
  String get settingsAutostartNote;

  /// Settings › Behaviour: title and switch name for staying in the system tray when the window is closed.
  ///
  /// In en, this message translates to:
  /// **'Keep running'**
  String get settingsKeepRunning;

  /// Settings › Behaviour: note under "Keep running" ("tray": the system tray / notification area / menu bar icons).
  ///
  /// In en, this message translates to:
  /// **'When the window is closed, Fogata stays in the tray, so friends can still call you.'**
  String get settingsKeepRunningNote;

  /// Settings › Behaviour: the Linux desktop has no system tray.
  ///
  /// In en, this message translates to:
  /// **'This desktop shows no tray icons, so closing the window quits Fogata.'**
  String get settingsNoTray;

  /// Settings › Network: title of the choice of how this computer helps other group members connect.
  ///
  /// In en, this message translates to:
  /// **'Helping members'**
  String get settingsHelping;

  /// Settings › Network: note under "Helping members".
  ///
  /// In en, this message translates to:
  /// **'How this computer helps the members of your groups.'**
  String get settingsHelpingNote;

  /// Settings › Network: option (segmented button): help members find each other and also pass their traffic. Short.
  ///
  /// In en, this message translates to:
  /// **'Help connect and relay'**
  String get settingsHelpRelay;

  /// Settings › Network: option (segmented button): only help members find each other. Short.
  ///
  /// In en, this message translates to:
  /// **'Help connect only'**
  String get settingsHelpConnect;

  /// Settings › Network: what "Help connect and relay" does ("upload": the internet upload bandwidth).
  ///
  /// In en, this message translates to:
  /// **'When two members cannot connect to each other, this computer passes their encrypted traffic (without seeing it), up to 2 GB per call. It uses your upload.'**
  String get settingsHelpRelayNote;

  /// Settings › Network: what "Help connect only" does.
  ///
  /// In en, this message translates to:
  /// **'This computer helps two members find each other but does not pass their traffic. If they cannot connect, they will not hear each other.'**
  String get settingsHelpConnectNote;

  /// Settings › Network: the relay choice applies after quitting from the tray icon and opening again.
  ///
  /// In en, this message translates to:
  /// **'The change applies once you quit Fogata from its tray icon and open it again.'**
  String get settingsRestartTray;

  /// Settings › Network: the relay choice applies after quitting and opening again.
  ///
  /// In en, this message translates to:
  /// **'The change applies once you quit Fogata and open it again.'**
  String get settingsRestart;

  /// Settings › Network: title of the network diagnosis block.
  ///
  /// In en, this message translates to:
  /// **'Network check'**
  String get settingsNetworkCheck;

  /// Settings › Network: note under "Network check".
  ///
  /// In en, this message translates to:
  /// **'How this computer sees the network and what it means for reaching your friends. Nothing leaves here: it only looks at what the connection already knows.'**
  String get settingsNetworkCheckNote;

  /// Settings › Network: button that runs the network check.
  ///
  /// In en, this message translates to:
  /// **'Check the network'**
  String get settingsCheckNetwork;

  /// Settings › Network: button that runs the network check again.
  ///
  /// In en, this message translates to:
  /// **'Check again'**
  String get settingsCheckAgain;

  /// Settings › Storage & downloads: title of the block showing the downloads folder.
  ///
  /// In en, this message translates to:
  /// **'Downloads'**
  String get settingsDownloads;

  /// Settings › Storage & downloads: note under "Downloads".
  ///
  /// In en, this message translates to:
  /// **'Where files downloaded from a room are saved.'**
  String get settingsDownloadsNote;

  /// Settings › Storage & downloads: small line with the app version and the folder where Fogata keeps its data.
  ///
  /// In en, this message translates to:
  /// **'Fogata {version} · Data in {path}'**
  String settingsVersionData(String version, String path);

  /// Settings › Storage & downloads: title of a warning that the data is stored unencrypted.
  ///
  /// In en, this message translates to:
  /// **'Not encrypted on this computer'**
  String get settingsNotEncrypted;

  /// Settings › Storage & downloads: warning text. {system} is linux (then it explains the Linux keychain: Secret Service, KWallet and GNOME Keyring are product names) or other. Keep the space at the start of each choice if the language separates sentences with spaces.
  ///
  /// In en, this message translates to:
  /// **'Your identity, friends and direct messages are stored on this computer unencrypted, because no system keychain was found to keep the key.{system, select, linux{ On Linux, the keychain is a running Secret Service, such as KWallet or GNOME Keyring: start or install one, and Fogata encrypts its data the next time it opens.} other{ Fogata tries again the next time it opens.}}'**
  String settingsNotEncryptedText(String system);

  /// Settings › Storage & downloads: title of a warning that encrypting the data on this computer began and did not end (shown instead of the one for no keychain).
  ///
  /// In en, this message translates to:
  /// **'Not all encrypted on this computer'**
  String get settingsNotAllEncrypted;

  /// Settings › Storage & downloads: warning text under settingsNotAllEncrypted. Also shown in the notices at start.
  ///
  /// In en, this message translates to:
  /// **'Part of the data on this computer (your identity, friends and direct messages) is not encrypted yet: Fogata is still encrypting it, or could not finish, as when the disk is full. It goes on each time Fogata opens.'**
  String get settingsNotAllEncryptedText;

  /// Settings › Storage & downloads: under settingsNotAllEncryptedText, when what keeps encrypting from finishing is a damaged file. {file} is the file's name.
  ///
  /// In en, this message translates to:
  /// **'{file} does not open with this computer\'s key: it was damaged on the disk. Encrypting cannot finish while it is there, and a copy of it from before encrypting stays unencrypted. Set it aside to finish: it is kept as it is. If it is your identity or your friends, Fogata takes back that copy and encrypts it; a group\'s or a chat\'s file is lost on this computer: the group has to let you in again, and the chat\'s earlier messages no longer show.'**
  String settingsDamagedSecret(String file);

  /// Settings › Storage & downloads: button under settingsDamagedSecret; sets the damaged file aside and starts Fogata again.
  ///
  /// In en, this message translates to:
  /// **'Set it aside and restart'**
  String get settingsSetDamagedAside;

  /// Settings › Storage & downloads: title of the block about disk use of direct messages.
  ///
  /// In en, this message translates to:
  /// **'Storage'**
  String get storageTitle;

  /// Settings › Storage & downloads: note under "Storage".
  ///
  /// In en, this message translates to:
  /// **'Your direct messages live only on your devices. Room chats are gone when the room closes, so only these grow.'**
  String get storageNote;

  /// Settings › Storage & downloads: label of the disk use of direct messages (followed by a size).
  ///
  /// In en, this message translates to:
  /// **'Direct messages'**
  String get storageDirect;

  /// Settings › Storage & downloads: after the size used by direct messages when no limit is set ("48 MB with no limit"). Lower case.
  ///
  /// In en, this message translates to:
  /// **'with no limit'**
  String get storageOfNoLimit;

  /// Settings › Storage & downloads: after the size used, the limit chosen ("48 MB of 1 GB"); {limit} is a size like "1 GB". Lower case.
  ///
  /// In en, this message translates to:
  /// **'of {limit}'**
  String storageOfLimit(String limit);

  /// Settings › Storage & downloads: legend of the usage bar; {size} is like "48.2 MB".
  ///
  /// In en, this message translates to:
  /// **'Pictures {size}'**
  String storageLegendPictures(String size);

  /// Settings › Storage & downloads: legend of the usage bar; {size} is like "48.2 MB".
  ///
  /// In en, this message translates to:
  /// **'Videos {size}'**
  String storageLegendVideos(String size);

  /// Settings › Storage & downloads: legend of the usage bar; {size} is like "48.2 MB".
  ///
  /// In en, this message translates to:
  /// **'Files {size}'**
  String storageLegendFiles(String size);

  /// Settings › Storage & downloads: legend of the usage bar (the text of messages); {size} is like "1.2 MB".
  ///
  /// In en, this message translates to:
  /// **'Messages {size}'**
  String storageLegendMessages(String size);

  /// Settings › Storage & downloads: title of the choice of how much disk space direct messages may take, their text and attachments together.
  ///
  /// In en, this message translates to:
  /// **'Space for direct messages'**
  String get storageRoom;

  /// Settings › Storage & downloads: note when no space limit is set.
  ///
  /// In en, this message translates to:
  /// **'Nothing is removed automatically.'**
  String get storageNoLimitNote;

  /// Settings › Storage & downloads: note when the space used is over the limit; {limit} is a size like "1 GB".
  ///
  /// In en, this message translates to:
  /// **'Over the limit: the oldest pictures, videos and files go, down to {limit}. Messages always stay.'**
  String storageOverNote(String limit);

  /// Settings › Storage & downloads: note about the space limit. The quoted text is what a removed picture shows in the chat: use the same words as that message.
  ///
  /// In en, this message translates to:
  /// **'Past the limit, the oldest pictures, videos and files go first. Messages always stay; in their place it says “Removed here to free space”.'**
  String get storageLimitNote;

  /// Settings › Storage & downloads: switch: pictures in direct messages download automatically.
  ///
  /// In en, this message translates to:
  /// **'Download pictures automatically'**
  String get storageAutoPictures;

  /// Settings › Storage & downloads: note under the automatic picture download switch.
  ///
  /// In en, this message translates to:
  /// **'Up to 10 MB each, as they arrive.'**
  String get storageAutoPicturesNote;

  /// Settings › Storage & downloads: switch: videos and files in direct messages download automatically.
  ///
  /// In en, this message translates to:
  /// **'Download videos and files automatically'**
  String get storageAutoFiles;

  /// Settings › Storage & downloads: what happens when the automatic download of videos and files is off.
  ///
  /// In en, this message translates to:
  /// **'Off: they come when you open them, while the sender is online.'**
  String get storageAutoFilesNote;

  /// Settings › Storage & downloads: title of the default timer after which messages of new direct chats disappear.
  ///
  /// In en, this message translates to:
  /// **'Disappearing messages in new chats'**
  String get storageTimer;

  /// Settings › Storage & downloads: note under the disappearing messages choice.
  ///
  /// In en, this message translates to:
  /// **'Each chat can change it; both people always see the same timer.'**
  String get storageTimerNote;

  /// Settings › Storage & downloads: very short option for a 24-hour disappearing timer (segmented button).
  ///
  /// In en, this message translates to:
  /// **'24 h'**
  String get storageTimer24h;

  /// Settings › Storage & downloads: title of the list of each direct chat's disk use.
  ///
  /// In en, this message translates to:
  /// **'By chat'**
  String get storageByChat;

  /// Settings › Storage & downloads: note under "By chat".
  ///
  /// In en, this message translates to:
  /// **'Clearing removes pictures, videos and files here. The messages stay, and the other person keeps their copy.'**
  String get storageByChatNote;

  /// Settings › Storage & downloads: a chat that holds no pictures, videos or files.
  ///
  /// In en, this message translates to:
  /// **'Messages only'**
  String get storageMessagesOnly;

  /// Settings › Storage & downloads: small button that removes a chat's pictures, videos and files from this computer.
  ///
  /// In en, this message translates to:
  /// **'Clear files'**
  String get storageClearFiles;

  /// Settings › Storage & downloads: tooltip of "Clear files"; {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'Clear the files of the chat with {name}'**
  String storageClearFilesTooltip(String name);

  /// Settings › Storage & downloads: short label shown after a chat's files were cleared.
  ///
  /// In en, this message translates to:
  /// **'Cleared'**
  String get storageCleared;

  /// Settings › Storage & downloads: question when choosing a limit smaller than what is used now; {limit} is a size like "1 GB".
  ///
  /// In en, this message translates to:
  /// **'Keep {limit} of pictures, videos and files?'**
  String storageConfirmTitle(String limit);

  /// Settings › Storage & downloads: text of the question above; {limit} is a size like "1 GB".
  ///
  /// In en, this message translates to:
  /// **'The oldest go now, down to {limit}, those of pinned messages last. Messages always stay, and your friends keep their copies.'**
  String storageConfirmText(String limit);

  /// Settings › Storage & downloads: button that confirms removing the oldest pictures, videos and files.
  ///
  /// In en, this message translates to:
  /// **'Remove the oldest'**
  String get storageRemoveOldest;

  /// Settings › Appearance: screen-reader label of the System theme choice (follows the computer's light or dark setting).
  ///
  /// In en, this message translates to:
  /// **'System theme, as the computer is set'**
  String get themePickerSystem;

  /// Settings › Appearance: screen-reader label of a theme choice; {name} is the theme's name like "Dark" or "Ember".
  ///
  /// In en, this message translates to:
  /// **'{name} theme'**
  String themePickerTheme(String name);

  /// Name of a colour theme: dark greys (the default). One word.
  ///
  /// In en, this message translates to:
  /// **'Dark'**
  String get themeDark;

  /// Name of a colour theme: true black. One word.
  ///
  /// In en, this message translates to:
  /// **'Midnight'**
  String get themeMidnight;

  /// Name of a colour theme: softer greys. One word.
  ///
  /// In en, this message translates to:
  /// **'Dim'**
  String get themeDim;

  /// Name of a colour theme: warm greys, like the glowing coals of a campfire. One word.
  ///
  /// In en, this message translates to:
  /// **'Ember'**
  String get themeEmber;

  /// Name of a colour theme: black and white with stronger lines and colours (high contrast). One word.
  ///
  /// In en, this message translates to:
  /// **'Contrast'**
  String get themeContrast;

  /// Name of a colour theme: light background. One word.
  ///
  /// In en, this message translates to:
  /// **'Light'**
  String get themeLight;

  /// Name of a colour theme choice that follows the computer between Dark and Light. One word.
  ///
  /// In en, this message translates to:
  /// **'System'**
  String get themeSystem;

  /// Name of a colour for the top of the person's card (tooltip in Settings › My profile): Violet. One or two words.
  ///
  /// In en, this message translates to:
  /// **'Violet'**
  String get themeBannerViolet;

  /// Name of a colour for the top of the person's card (tooltip in Settings › My profile): Blue. One or two words.
  ///
  /// In en, this message translates to:
  /// **'Blue'**
  String get themeBannerBlue;

  /// Name of a colour for the top of the person's card (tooltip in Settings › My profile): Green. One or two words.
  ///
  /// In en, this message translates to:
  /// **'Green'**
  String get themeBannerGreen;

  /// Name of a colour for the top of the person's card (tooltip in Settings › My profile): Clay (a brick-red brown). One or two words.
  ///
  /// In en, this message translates to:
  /// **'Clay'**
  String get themeBannerClay;

  /// Name of a colour for the top of the person's card (tooltip in Settings › My profile): Raspberry (a deep pink-red). One or two words.
  ///
  /// In en, this message translates to:
  /// **'Raspberry'**
  String get themeBannerRaspberry;

  /// Name of a colour for the top of the person's card (tooltip in Settings › My profile): Mustard (a dull yellow). One or two words.
  ///
  /// In en, this message translates to:
  /// **'Mustard'**
  String get themeBannerMustard;

  /// Name of a colour for the top of the person's card (tooltip in Settings › My profile): Slate (a blue-grey). One or two words.
  ///
  /// In en, this message translates to:
  /// **'Slate'**
  String get themeBannerSlate;

  /// Name of a colour for the top of the person's card (tooltip in Settings › My profile): Night (near black). One or two words.
  ///
  /// In en, this message translates to:
  /// **'Night'**
  String get themeBannerNight;

  /// Tooltip and screen-reader label of a copy button. {what} says which secret it copies: qr (a QR code; English says "QR code"), link (a friend link) or code (a friend code).
  ///
  /// In en, this message translates to:
  /// **'{what, select, qr{Copy QR code} link{Copy link} code{Copy code} other{Copy}}'**
  String widgetsCopyWhat(String what);

  /// Tooltip of the eye button that shows a blurred secret for ten seconds. {what} says which: qr (a QR code; English says "QR code"), link (a friend link) or code (a friend code).
  ///
  /// In en, this message translates to:
  /// **'{what, select, qr{Show QR code} link{Show link} code{Show code} other{Show}}'**
  String widgetsShowWhat(String what);

  /// Tooltip of the eye button that blurs a shown secret (a link, code or QR) again.
  ///
  /// In en, this message translates to:
  /// **'Hide'**
  String get widgetsHide;

  /// Toast when the file manager could not be opened on a file; {path} is where the file is.
  ///
  /// In en, this message translates to:
  /// **'Could not open its folder. It is at {path}'**
  String revealFailed(String path);

  /// Emoji picker: name of a group of emoji (faces, people and gestures), as a heading and a tooltip. Short.
  ///
  /// In en, this message translates to:
  /// **'People'**
  String get emojiGroupPeople;

  /// Emoji picker: name of a group of emoji (animals and plants), as a heading and a tooltip. Short.
  ///
  /// In en, this message translates to:
  /// **'Nature'**
  String get emojiGroupNature;

  /// Emoji picker: name of a group of emoji (food and drink), as a heading and a tooltip. Short.
  ///
  /// In en, this message translates to:
  /// **'Food & drink'**
  String get emojiGroupFood;

  /// Emoji picker: name of a group of emoji (sports, games, events), as a heading and a tooltip. Short.
  ///
  /// In en, this message translates to:
  /// **'Activities'**
  String get emojiGroupActivities;

  /// Emoji picker: name of a group of emoji (vehicles, places, weather), as a heading and a tooltip. Short.
  ///
  /// In en, this message translates to:
  /// **'Travel & places'**
  String get emojiGroupTravel;

  /// Emoji picker: name of a group of emoji (everyday objects), as a heading and a tooltip. Short.
  ///
  /// In en, this message translates to:
  /// **'Objects'**
  String get emojiGroupObjects;

  /// Emoji picker: name of a group of emoji (hearts, signs and symbols), as a heading and a tooltip. Short.
  ///
  /// In en, this message translates to:
  /// **'Symbols'**
  String get emojiGroupSymbols;

  /// Emoji picker: name of a group of emoji (flags), as a heading and a tooltip. Short.
  ///
  /// In en, this message translates to:
  /// **'Flags'**
  String get emojiGroupFlags;

  /// Toast when an emoji cannot be added because the message is already as long as allowed.
  ///
  /// In en, this message translates to:
  /// **'The message is at its limit.'**
  String get emojiPickerLimit;

  /// Tooltip of the smiley button that opens the emoji picker beside the message box.
  ///
  /// In en, this message translates to:
  /// **'Emoji'**
  String get emojiPickerButton;

  /// Emoji picker: heading above the emoji that match the search.
  ///
  /// In en, this message translates to:
  /// **'Results'**
  String get emojiPickerResults;

  /// Emoji picker: heading and tooltip of the emoji used last.
  ///
  /// In en, this message translates to:
  /// **'Recently used'**
  String get emojiPickerRecent;

  /// Emoji picker: placeholder of the search field. Emoji names are searched in English.
  ///
  /// In en, this message translates to:
  /// **'Search emoji'**
  String get emojiPickerSearch;

  /// Emoji picker: tooltip and label of the skin tone button and row.
  ///
  /// In en, this message translates to:
  /// **'Skin tone'**
  String get emojiPickerSkinTone;

  /// Emoji picker: nothing matches the search.
  ///
  /// In en, this message translates to:
  /// **'No emoji found'**
  String get emojiPickerNone;

  /// Emoji picker, Linux: no colour emoji font installed. "Noto Color Emoji" is a font name.
  ///
  /// In en, this message translates to:
  /// **'This computer has no colour emoji font, so emoji may look plain or empty. Installing Noto Color Emoji fixes it.'**
  String get emojiPickerNoFont;

  /// Emoji picker: screen-reader label of the default (yellow) skin tone.
  ///
  /// In en, this message translates to:
  /// **'Default skin tone'**
  String get emojiPickerDefaultTone;

  /// Emoji picker: screen-reader label of one of the five skin tones, from 1 (lightest) to 5 (darkest).
  ///
  /// In en, this message translates to:
  /// **'Skin tone {n}'**
  String emojiPickerTone(int n);

  /// Name of an icon a group can have (tooltip when choosing the group's icon): mountain.
  ///
  /// In en, this message translates to:
  /// **'Mountain'**
  String get iconsGroupMountain;

  /// Name of an icon a group can have (tooltip when choosing the group's icon): star.
  ///
  /// In en, this message translates to:
  /// **'Star'**
  String get iconsGroupStar;

  /// Name of an icon a group can have (tooltip when choosing the group's icon): game controller.
  ///
  /// In en, this message translates to:
  /// **'Game controller'**
  String get iconsGroupGameController;

  /// Name of an icon a group can have (tooltip when choosing the group's icon): music.
  ///
  /// In en, this message translates to:
  /// **'Music'**
  String get iconsGroupMusic;

  /// Name of an icon a group can have (tooltip when choosing the group's icon): book.
  ///
  /// In en, this message translates to:
  /// **'Book'**
  String get iconsGroupBook;

  /// Name of an icon a group can have (tooltip when choosing the group's icon): waves.
  ///
  /// In en, this message translates to:
  /// **'Waves'**
  String get iconsGroupWaves;

  /// Name of an icon a group can have (tooltip when choosing the group's icon): coffee.
  ///
  /// In en, this message translates to:
  /// **'Coffee'**
  String get iconsGroupCoffee;

  /// Name of an icon a group can have (tooltip when choosing the group's icon): heart.
  ///
  /// In en, this message translates to:
  /// **'Heart'**
  String get iconsGroupHeart;

  /// Name of an icon a group can have (tooltip when choosing the group's icon): film.
  ///
  /// In en, this message translates to:
  /// **'Film'**
  String get iconsGroupFilm;

  /// New group dialog: error toast when the picture chosen from the computer cannot be opened.
  ///
  /// In en, this message translates to:
  /// **'That picture could not be read.'**
  String get newGroupPictureUnreadable;

  /// Toast after a new group was created. {name} is the group name, shown in quotes.
  ///
  /// In en, this message translates to:
  /// **'Group “{name}” created. Its members get it when they come online.'**
  String newGroupMade(String name);

  /// New group dialog: its title.
  ///
  /// In en, this message translates to:
  /// **'New group'**
  String get newGroupTitle;

  /// New group dialog: line under the title. A group stays; its voice rooms open and close (they "light up" when someone enters).
  ///
  /// In en, this message translates to:
  /// **'The group is permanent. Its rooms are not.'**
  String get newGroupSubtitle;

  /// New group dialog: label of the name field (also read by screen readers).
  ///
  /// In en, this message translates to:
  /// **'Group name'**
  String get newGroupName;

  /// New group dialog: example group name shown greyed in the empty name field (a group of friends who cycle on Sundays). Any short, natural group name works.
  ///
  /// In en, this message translates to:
  /// **'Sunday ride'**
  String get newGroupNameExample;

  /// New group dialog: label over the row of icons, beside the button that picks a picture.
  ///
  /// In en, this message translates to:
  /// **'Icon or picture'**
  String get newGroupIconOrPicture;

  /// New group dialog: button that opens a file dialog to pick the group picture. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Choose from computer'**
  String get newGroupChooseFromComputer;

  /// New group dialog: label over the banner choice (a wide picture at the top of the group's column).
  ///
  /// In en, this message translates to:
  /// **'Banner (optional)'**
  String get newGroupBannerOptional;

  /// New group dialog: label over the list of friends to add as members.
  ///
  /// In en, this message translates to:
  /// **'Members'**
  String get newGroupMembers;

  /// New group dialog: how many friends are ticked to be members.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 selected} other{{n} selected}}'**
  String newGroupSelected(int n);

  /// New group dialog: greyed hint in the empty search field over the friends list.
  ///
  /// In en, this message translates to:
  /// **'Search your friends'**
  String get newGroupSearchPlaceholder;

  /// New group dialog: screen-reader label of the search field over the friends list.
  ///
  /// In en, this message translates to:
  /// **'Search friends'**
  String get newGroupSearchLabel;

  /// New group dialog: shown in the friends list when the person has no friends yet.
  ///
  /// In en, this message translates to:
  /// **'No friends yet. You can make the group and add them later, or invite people with a link.'**
  String get newGroupNoFriends;

  /// New group dialog footer: the group lives on its members' computers (no server).
  ///
  /// In en, this message translates to:
  /// **'Kept on every member’s device.'**
  String get newGroupKeptOnDevices;

  /// New group dialog: main button that makes the group.
  ///
  /// In en, this message translates to:
  /// **'Create group'**
  String get newGroupCreate;

  /// Tooltip of the gear button at the top of a group's side column (opens the group's settings).
  ///
  /// In en, this message translates to:
  /// **'Group settings'**
  String get groupBannerSettings;

  /// Error toast when the banner picture chosen from the computer cannot be opened.
  ///
  /// In en, this message translates to:
  /// **'That picture could not be read.'**
  String get groupBannerPictureUnreadable;

  /// Button that opens a file dialog to pick the group's banner (a wide picture at the top of its column). Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Choose a banner'**
  String get groupBannerChoose;

  /// Button that picks a different banner picture, replacing the current one. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Another banner'**
  String get groupBannerAnother;

  /// Button that removes the group's banner. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'No banner'**
  String get groupBannerNone;

  /// Note beside the banner choice: where the banner shows and which files it takes (PNG and JPEG pictures, or an animated GIF or WebP up to 550 KB). Keep the format names and the size.
  ///
  /// In en, this message translates to:
  /// **'Shown at the top of the group’s column, under its name. PNG or JPEG, or an animated GIF or WebP up to 550 KB.'**
  String get groupBannerNote;

  /// Home page: its big title.
  ///
  /// In en, this message translates to:
  /// **'Home'**
  String get homeTitle;

  /// Home page: line under the title. A group's voice room is "lit" (live) while someone is in it, like a campfire (Fogata means campfire).
  ///
  /// In en, this message translates to:
  /// **'Rooms light up when someone enters and go out when everyone leaves.'**
  String get homeSubtitle;

  /// Home page: button that opens the Friends page to add a friend.
  ///
  /// In en, this message translates to:
  /// **'Add a friend'**
  String get homeAddFriend;

  /// Home page: main button that opens the new group dialog. Short.
  ///
  /// In en, this message translates to:
  /// **'New group'**
  String get homeNewGroup;

  /// Home page: heading over the groups whose voice room has people in it now, beside their count.
  ///
  /// In en, this message translates to:
  /// **'Live'**
  String get homeLive;

  /// Home page: heading over the groups with no voice room open right now.
  ///
  /// In en, this message translates to:
  /// **'No room'**
  String get homeNoRoom;

  /// Home page: note beside the "No room" heading: opening a group's room makes it live.
  ///
  /// In en, this message translates to:
  /// **'Opening a group’s room lights it up'**
  String get homeNoRoomNote;

  /// Home page, a group invitation card: which friend added this person to the group, and how many members it has.
  ///
  /// In en, this message translates to:
  /// **'{name} added you · {n, plural, =1{1 member} other{{n} members}}'**
  String homeInviteBy(String name, int n);

  /// Home page, a group invitation card: nothing happens with the group until the person joins it.
  ///
  /// In en, this message translates to:
  /// **'Until you join, this computer neither connects to them nor sends them anything.'**
  String get homeInviteNote;

  /// Home page, a group invitation card: button to join the group. Short.
  ///
  /// In en, this message translates to:
  /// **'Join'**
  String get homeInviteJoin;

  /// Home page: title of the empty state when the person has no groups.
  ///
  /// In en, this message translates to:
  /// **'No groups yet'**
  String get homeNoGroups;

  /// Home page: text of the empty state when the person has no groups.
  ///
  /// In en, this message translates to:
  /// **'Add your friends and make a group with them. The group is permanent; its rooms are not.'**
  String get homeNoGroupsNote;

  /// Home page, a live room card: the person's own name in the list of who is in the room.
  ///
  /// In en, this message translates to:
  /// **'{name} (you)'**
  String homePersonYou(String name);

  /// Home page, a live room card: how many members the group has.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{1 member} other{{n} members}}'**
  String homeMembers(int n);

  /// Home page, a live room card: small label when this person is in that room. Short: fits a small pill.
  ///
  /// In en, this message translates to:
  /// **'You are in'**
  String get homeYouAreIn;

  /// Home page, a live room card: tooltip of the microphone button while the microphone is on.
  ///
  /// In en, this message translates to:
  /// **'Mute microphone'**
  String get homeMuteMic;

  /// Home page, a live room card: tooltip of the microphone button while the microphone is muted.
  ///
  /// In en, this message translates to:
  /// **'Unmute microphone'**
  String get homeUnmuteMic;

  /// Home page, a live room card: tooltip of the camera button while the camera is on.
  ///
  /// In en, this message translates to:
  /// **'Turn the camera off'**
  String get homeCameraOff;

  /// Home page, a live room card: tooltip of the camera button while the camera is off.
  ///
  /// In en, this message translates to:
  /// **'Turn the camera on'**
  String get homeCameraOn;

  /// Home page: tooltip of the button that opens the group's members.
  ///
  /// In en, this message translates to:
  /// **'Members'**
  String get homeMembersTooltip;

  /// Home page, a live room card: button that goes back to the room this person is in. Short.
  ///
  /// In en, this message translates to:
  /// **'Return to room'**
  String get homeReturnToRoom;

  /// Home page, a live room card: button that enters the live voice room. Short.
  ///
  /// In en, this message translates to:
  /// **'Join'**
  String get homeJoinRoom;

  /// Home page, a group row: how many members the group has and how many of the others are online now.
  ///
  /// In en, this message translates to:
  /// **'{members, plural, =1{1 member} other{{members} members}} · {online, plural, =0{nobody else online} other{{online} online}}'**
  String homeMembersOnline(int members, int online);

  /// Home page, a group row: the group never had a voice room open.
  ///
  /// In en, this message translates to:
  /// **'No rooms yet'**
  String get homeNoRoomsYet;

  /// Home page, a group row: when its voice room was last open. {when} is a relative time like "5 min ago", "yesterday" or "3 days ago".
  ///
  /// In en, this message translates to:
  /// **'Last room {when}'**
  String homeLastRoom(String when);

  /// Home page, a group row: button that opens (lights up) the group's voice room and enters it. Short.
  ///
  /// In en, this message translates to:
  /// **'Open room'**
  String get homeOpenRoom;

  /// Members page: shown when the group was left or removed while its page was open.
  ///
  /// In en, this message translates to:
  /// **'This group is no longer here.'**
  String get membersGroupGone;

  /// Members page, under the group name: how many members the group has and how many of the others are online now.
  ///
  /// In en, this message translates to:
  /// **'{members, plural, =1{1 member} other{{members} members}} · {online, plural, =0{nobody else online} other{{online} online}}'**
  String membersSummary(int members, int online);

  /// Members page, under the group name: how many members, how many others are online, and when its voice room was last open ({when} is a relative time like "5 min ago" or "yesterday").
  ///
  /// In en, this message translates to:
  /// **'{members, plural, =1{1 member} other{{members} members}} · {online, plural, =0{nobody else online} other{{online} online}} · last room {when}'**
  String membersSummaryLastRoom(int members, int online, String when);

  /// Members page: button that copies a one-time link that lets someone join the group.
  ///
  /// In en, this message translates to:
  /// **'Copy invite link'**
  String get membersCopyInviteLink;

  /// Members page: tooltip of the Copy invite link button. The link can be used once, for 24 hours, and a member must let the newcomer in. {who} says who lets the person in: anyone (any member), first (only the first member) or other (one of the first {n} members, {n} is 2 or more).
  ///
  /// In en, this message translates to:
  /// **'Works once, for 24 hours, and {who, select, anyone{someone in the group} first{the first in the group} other{one of the first {n} in the group}} lets them in'**
  String membersInviteLinkTooltip(String who, int n);

  /// Toast after the invite link was copied. {who} says who lets the person in: anyone (any member), first (only the first member) or other (one of the first {n} members, {n} is 2 or more).
  ///
  /// In en, this message translates to:
  /// **'Invite link copied. It works once, for 24 hours, and {who, select, anyone{someone in the group} first{the first in the group} other{one of the first {n} in the group}} lets them in.'**
  String membersInviteLinkCopied(String who, int n);

  /// Members page: button that opens a list of friends to invite into the group.
  ///
  /// In en, this message translates to:
  /// **'Invite friends'**
  String get membersInviteFriends;

  /// Members page: main button when this person is in the group's voice room: goes back to it.
  ///
  /// In en, this message translates to:
  /// **'Back to the room'**
  String get membersBackToRoom;

  /// Members page: main button when the group's voice room is not live: opens (lights up) the room and enters it.
  ///
  /// In en, this message translates to:
  /// **'Open room'**
  String get membersOpenRoom;

  /// Members page: main button when the group's voice room is live: enters it.
  ///
  /// In en, this message translates to:
  /// **'Enter the room'**
  String get membersEnterRoom;

  /// Members page: heading of the list of members.
  ///
  /// In en, this message translates to:
  /// **'Members'**
  String get membersTitle;

  /// Members page: note beside the heading: members are listed in rank order, and each member can manage (move, time out, remove) only those listed below them.
  ///
  /// In en, this message translates to:
  /// **'In order: each one acts on those below'**
  String get membersOrderNote;

  /// Members page: button that opens the group's settings.
  ///
  /// In en, this message translates to:
  /// **'Group settings'**
  String get membersGroupSettings;

  /// Members page footer, for someone who cannot change the group: only the first {n} members in the list change it (name, rooms, invites).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{Only the first one can change the group.} other{Only the first {n} can change the group.}}'**
  String membersManagersNote(int n);

  /// Members page: red button to leave the group. Also the confirm button of the question it asks.
  ///
  /// In en, this message translates to:
  /// **'Leave group'**
  String get membersLeaveGroup;

  /// Invite friends dialog: title. {group} is the group's name, in quotes.
  ///
  /// In en, this message translates to:
  /// **'Invite friends to “{group}”'**
  String membersInviteTitle(String group);

  /// Invite friends dialog: line under the title.
  ///
  /// In en, this message translates to:
  /// **'They get an invitation and choose whether to join.'**
  String get membersInviteNote;

  /// Members page, a member row: how this person is reached, for their own row. Lower case, short.
  ///
  /// In en, this message translates to:
  /// **'you'**
  String get membersPresenceYou;

  /// Members page, a member row: the member is online, and how this computer reaches them ({how} is a short phrase like "direct", "on this network" or "through Marta"). Lower case.
  ///
  /// In en, this message translates to:
  /// **'online · {how}'**
  String membersPresenceOnline(String how);

  /// Members page, a member row: the member is online but has not used their computer for 10 minutes or more, and how this computer reaches them ({how} is a short phrase like "direct", "on this network" or "through Marta"). Lower case.
  ///
  /// In en, this message translates to:
  /// **'away · {how}'**
  String membersPresenceAway(String how);

  /// Members page, a member row: the member is offline. Lower case.
  ///
  /// In en, this message translates to:
  /// **'not online now'**
  String get membersPresenceOffline;

  /// Members page: this person's own name in the list of members.
  ///
  /// In en, this message translates to:
  /// **'{name} (you)'**
  String membersYou(String name);

  /// Members page, a member row: the member has not received the group's encryption key yet. Lower case, short.
  ///
  /// In en, this message translates to:
  /// **'waiting for the key'**
  String get membersWaitingForKey;

  /// Members page: tooltip on a member's short code, with their full fingerprint (a long code that identifies their device; keep it as is).
  ///
  /// In en, this message translates to:
  /// **'Fingerprint {fingerprint}'**
  String membersFingerprint(String fingerprint);

  /// Members page: tooltip of the "···" button on a member's row, which opens a menu of actions on them.
  ///
  /// In en, this message translates to:
  /// **'Options for {name}'**
  String membersOptionsFor(String name);

  /// Members page, a member's menu: move the member up the list to just below this person.
  ///
  /// In en, this message translates to:
  /// **'Move up below me'**
  String get membersMoveUp;

  /// Members page, a member's menu: move the member to the bottom of the list.
  ///
  /// In en, this message translates to:
  /// **'Move to the end'**
  String get membersMoveToEnd;

  /// Members page, a member's menu: keep the member out of a voice room for a while (opens a dialog to choose how long).
  ///
  /// In en, this message translates to:
  /// **'Time out of a room…'**
  String get membersTimeOut;

  /// Members page, a member's menu: take the member out of the group (asks first).
  ///
  /// In en, this message translates to:
  /// **'Remove from the group'**
  String get membersRemoveFromGroup;

  /// Question before removing a member from the group: its title.
  ///
  /// In en, this message translates to:
  /// **'Remove {name}?'**
  String membersRemoveTitle(String name);

  /// Question before removing a member: what happens (the group changes its encryption key; messages they already saw stay with them).
  ///
  /// In en, this message translates to:
  /// **'{name} leaves the group and the group makes a new key. What they already saw, they keep.'**
  String membersRemoveText(String name);

  /// Small label (with a crown) on the first member of a group, who has every right in it. Short: fits a small pill.
  ///
  /// In en, this message translates to:
  /// **'First'**
  String get membersFirst;

  /// Time out dialog: title. Keeping {name} (a member) out of a voice room for a while.
  ///
  /// In en, this message translates to:
  /// **'Time {name} out'**
  String membersTimeOutTitle(String name);

  /// Time out dialog: what happens to the member.
  ///
  /// In en, this message translates to:
  /// **'{name} leaves the room and cannot come back until the time is up or someone above them lets them back.'**
  String membersTimeOutText(String name);

  /// Time out dialog: red button: keep the member out with no time limit, until someone lets them back. Short: fits a button.
  ///
  /// In en, this message translates to:
  /// **'Until let back'**
  String get membersUntilLetBack;

  /// Question before leaving a group: its title. {group} is the group's name, in quotes.
  ///
  /// In en, this message translates to:
  /// **'Leave “{group}”?'**
  String membersLeaveTitle(String group);

  /// Question before leaving a group: what happens.
  ///
  /// In en, this message translates to:
  /// **'You stop seeing its rooms and the others stop seeing you in the group. The group makes a new key without you. Coming back takes someone in the group letting you in again.'**
  String get membersLeaveText;

  /// Question before leaving a group, when this person is its first member: what happens, and that {name} (the next member) becomes the first. Same text as membersLeaveText plus the last sentence.
  ///
  /// In en, this message translates to:
  /// **'You stop seeing its rooms and the others stop seeing you in the group. The group makes a new key without you. Coming back takes someone in the group letting you in again. {name} becomes first.'**
  String membersLeaveTextFirst(String name);

  /// Group settings: a tab in the list on the left (also the title of that tab's page). Short. Name, picture and banner.
  ///
  /// In en, this message translates to:
  /// **'General'**
  String get groupSettingsTabGeneral;

  /// Group settings: a tab in the list on the left (also the title of that tab's page). Short. The group's voice rooms.
  ///
  /// In en, this message translates to:
  /// **'Voice rooms'**
  String get groupSettingsTabVoiceRooms;

  /// Group settings: a tab in the list on the left (also the title of that tab's page). Short. The members in their order.
  ///
  /// In en, this message translates to:
  /// **'Members'**
  String get groupSettingsTabMembers;

  /// Group settings: a tab in the list on the left (also the title of that tab's page). Short. Invitations and invite links.
  ///
  /// In en, this message translates to:
  /// **'Invite'**
  String get groupSettingsTabInvite;

  /// Group settings: a tab in the list on the left (also the title of that tab's page). Short. How many people the group and each room take, and who manages it.
  ///
  /// In en, this message translates to:
  /// **'Limits'**
  String get groupSettingsTabLimits;

  /// Group settings: a tab in the list on the left (also the title of that tab's page). Short. What happened in the group (who joined, renamed, …).
  ///
  /// In en, this message translates to:
  /// **'Activity'**
  String get groupSettingsTabActivity;

  /// Group settings, under the group name at the top left: this person is the group's first member.
  ///
  /// In en, this message translates to:
  /// **'First in this group'**
  String get groupSettingsFirstHere;

  /// Group settings, under the group name: this person's place in the group's order. {place} is an ordinal like "2nd" (in the language's own ordinal form).
  ///
  /// In en, this message translates to:
  /// **'{place} in this group'**
  String groupSettingsPlaceHere(String place);

  /// Group settings: red item at the bottom of the tab list that leaves the group (asks first).
  ///
  /// In en, this message translates to:
  /// **'Leave group'**
  String get groupSettingsLeave;

  /// Group settings: title of the Invite tab's page.
  ///
  /// In en, this message translates to:
  /// **'Invite people'**
  String get groupSettingsInviteTitle;

  /// Group settings: tooltip of the close button (top right).
  ///
  /// In en, this message translates to:
  /// **'Close group settings'**
  String get groupSettingsClose;

  /// Group settings › General: small heading over the group's picture.
  ///
  /// In en, this message translates to:
  /// **'Picture'**
  String get groupSettingsPicture;

  /// Group settings › General: button that opens a file dialog to pick the group picture. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Choose from computer'**
  String get groupSettingsChooseFromComputer;

  /// Group settings › General: button that drops the picture and shows the chosen icon instead. Short.
  ///
  /// In en, this message translates to:
  /// **'Use an icon'**
  String get groupSettingsUseIcon;

  /// Group settings › General: note under the picture buttons: which files work and what is done to them.
  ///
  /// In en, this message translates to:
  /// **'PNG, JPEG or WebP, cut to a square and made smaller like profile pictures, or an animated GIF or WebP up to 550 KB.'**
  String get groupSettingsPictureNote;

  /// Group settings › General: small label over the row of icons that can stand for the group instead of a picture.
  ///
  /// In en, this message translates to:
  /// **'Or an icon'**
  String get groupSettingsOrIcon;

  /// Group settings › General: small heading over the banner choice (a wide picture at the top of the group's column).
  ///
  /// In en, this message translates to:
  /// **'Banner'**
  String get groupSettingsBanner;

  /// Group settings › General: small heading over the group name field.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get groupSettingsName;

  /// Group settings › General: screen-reader label of the group name field.
  ///
  /// In en, this message translates to:
  /// **'Group name'**
  String get groupSettingsNameField;

  /// Group settings › General: note under the name field.
  ///
  /// In en, this message translates to:
  /// **'Everyone in the group sees the new name and picture.'**
  String get groupSettingsNameNote;

  /// Group settings › Voice rooms: explanation at the top of the tab.
  ///
  /// In en, this message translates to:
  /// **'Each voice room has its own people, chat, files and calls: those in one do not connect with those in another. Someone is in one room at a time.'**
  String get groupSettingsRoomsIntro;

  /// Group settings › Voice rooms: screen-reader label of a voice room's name field.
  ///
  /// In en, this message translates to:
  /// **'Room name'**
  String get groupSettingsRoomName;

  /// Group settings › Voice rooms: small button on a room's row saying everyone in the group can enter it; it opens the choice. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Everyone'**
  String get groupSettingsAccessEveryone;

  /// Group settings › Voice rooms: small button on a room's row saying only some members can enter it; it opens the choice. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Only some'**
  String get groupSettingsAccessSome;

  /// Group settings › Voice rooms: tooltip of the access button and heading of the access choice. {room} is the voice room's name.
  ///
  /// In en, this message translates to:
  /// **'Who can enter {room}'**
  String groupSettingsWhoCanEnter(String room);

  /// Group settings › Voice rooms: tooltip of the (disabled) delete button of the first room, which cannot be deleted.
  ///
  /// In en, this message translates to:
  /// **'The first room stays'**
  String get groupSettingsFirstRoomStays;

  /// Group settings › Voice rooms: tooltip of the (disabled) delete button of a room with people in it.
  ///
  /// In en, this message translates to:
  /// **'People are inside'**
  String get groupSettingsPeopleInside;

  /// Group settings › Voice rooms: tooltip of a room's delete (trash) button.
  ///
  /// In en, this message translates to:
  /// **'Delete room'**
  String get groupSettingsDeleteRoom;

  /// Group settings › Voice rooms: button that adds a new voice room to the group.
  ///
  /// In en, this message translates to:
  /// **'Add voice room'**
  String get groupSettingsAddRoom;

  /// The name a new voice room gets, with its number (like "Room 2"); everyone in the group sees it until someone renames it.
  ///
  /// In en, this message translates to:
  /// **'Room {n}'**
  String groupSettingsNewRoomName(int n);

  /// Group settings › Voice rooms: note under the list. {rooms} is the most voice rooms a group has (10); {people} how many people each takes, set in the Limits tab (keep "Limits" as that tab's name).
  ///
  /// In en, this message translates to:
  /// **'Up to {rooms} rooms, and {people} people in each (Limits). A room with people inside cannot be deleted.'**
  String groupSettingsRoomsNote(int rooms, int people);

  /// Group settings › Voice rooms: heading over the list of members timed out of a room right now.
  ///
  /// In en, this message translates to:
  /// **'Kept out'**
  String get groupSettingsKeptOut;

  /// Group settings › Voice rooms, who can enter a room: first choice.
  ///
  /// In en, this message translates to:
  /// **'Everyone in the group'**
  String get groupSettingsAccessEveryoneLong;

  /// Group settings › Voice rooms, who can enter a room: second choice, which shows a list of members to tick.
  ///
  /// In en, this message translates to:
  /// **'Only the members I choose'**
  String get groupSettingsAccessChosen;

  /// Group settings › Voice rooms, who can enter a room: note under the members list ("above you" = earlier in the group's order).
  ///
  /// In en, this message translates to:
  /// **'You and those above you can always enter. Someone left out sees the room with a lock.'**
  String get groupSettingsAccessNote;

  /// Group settings: this person's own name in lists of members.
  ///
  /// In en, this message translates to:
  /// **'{name} (you)'**
  String groupSettingsYou(String name);

  /// Group settings › Voice rooms › Kept out: after a timed-out member's name, which voice room {room} they are kept out of and who {by} did it. Starts with " · " because it follows the name.
  ///
  /// In en, this message translates to:
  /// **' · {room} · by {by}'**
  String groupSettingsKeptOutBy(String room, String by);

  /// Group settings › Voice rooms › Kept out: how long until a timed-out member can come back. {time} is a short duration like "58 min", "3 h" or "2 days".
  ///
  /// In en, this message translates to:
  /// **'{time} left'**
  String groupSettingsTimeLeft(String time);

  /// Group settings › Voice rooms › Kept out: the member is kept out with no time limit, until someone lets them back. Short.
  ///
  /// In en, this message translates to:
  /// **'Until let back'**
  String get groupSettingsUntilLetBack;

  /// Group settings › Voice rooms › Kept out: button that ends a member's time out at once. Short: fits a small button.
  ///
  /// In en, this message translates to:
  /// **'Let back'**
  String get groupSettingsLetBack;

  /// Group settings › Members, for someone who does not manage the group: only the first {n} members in the order change it; everyone acts on those listed below them.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{Only the first one can change the group: its name, rooms, invitations and limits. You act on those below you.} other{Only the first {n} can change the group: its name, rooms, invitations and limits. You act on those below you.}}'**
  String groupSettingsManagersNote(int n);

  /// Group settings › Members: how many members there are of the most the group takes, over the list in their order.
  ///
  /// In en, this message translates to:
  /// **'{n} of {max, plural, =1{1 member} other{{max} members}}, in order'**
  String groupSettingsMembersCount(int n, int max);

  /// Group settings › Members: note under the list explaining the order.
  ///
  /// In en, this message translates to:
  /// **'Everyone acts on the people below them: moves them up to just below themselves or down to the end, times them out of a room, or removes them. Whoever comes in starts at the end.'**
  String get groupSettingsMembersNote;

  /// Group settings › Members (for managers): heading of the choice of who manages the group.
  ///
  /// In en, this message translates to:
  /// **'Who manages this group'**
  String get groupSettingsWhoManages;

  /// Group settings › Members › Who manages this group: first choice: every member manages it.
  ///
  /// In en, this message translates to:
  /// **'Everyone'**
  String get groupSettingsManagersEveryone;

  /// Group settings › Members › Who manages this group: second choice: only the first {n} members in the group's order manage it (a slider sets {n}).
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{The first one} other{The first {n}}}'**
  String groupSettingsManagersFirstN(int n);

  /// Group settings › Members › Who manages this group: the number chosen ({n}) is higher than how many members the group has now ({members}), so for now every member manages it.
  ///
  /// In en, this message translates to:
  /// **'Set to the first {n}, more than the {members, plural, =1{1} other{{members}}} in the group now: for now that is everyone.'**
  String groupSettingsManagersMore(int n, int members);

  /// Group settings › Members › Who manages this group: note when everyone manages it.
  ///
  /// In en, this message translates to:
  /// **'Those who manage change the name, pictures, rooms and limits, and let people in. With everyone, the group keeps going whoever loses their device.'**
  String get groupSettingsManagersEveryoneNote;

  /// Group settings › Members › Who manages this group: warning when only the first {n} members manage it.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =1{Only the first one changes the name, pictures, rooms and limits, and lets people in. If that device is lost, nobody can manage the group any more: Fogata has no server that could fix it.} other{Only the first {n} change the name, pictures, rooms and limits, and let people in. If they all lose their devices, nobody can manage the group any more: Fogata has no server that could fix it.}}'**
  String groupSettingsManagersFirstNote(int n);

  /// Group settings › Members: small button on a member's row: moves them up to just below this person. Short.
  ///
  /// In en, this message translates to:
  /// **'Move up'**
  String get groupSettingsMoveUp;

  /// Group settings › Members: tooltip of the Move up button.
  ///
  /// In en, this message translates to:
  /// **'Move {name} up to just below you'**
  String groupSettingsMoveUpTooltip(String name);

  /// Group settings › Members: small button on a member's row: moves them to the bottom of the order. Short.
  ///
  /// In en, this message translates to:
  /// **'To the end'**
  String get groupSettingsToEnd;

  /// Group settings › Members: tooltip of the To the end button.
  ///
  /// In en, this message translates to:
  /// **'Move {name} to the end'**
  String groupSettingsToEndTooltip(String name);

  /// Group settings › Members: tooltip of the timer button on a member's row: keep them out of a voice room for a while.
  ///
  /// In en, this message translates to:
  /// **'Time out of a room'**
  String get groupSettingsTimeOut;

  /// Group settings › Members: tooltip of the button that removes a member from the group (asks first).
  ///
  /// In en, this message translates to:
  /// **'Remove from the group'**
  String get groupSettingsRemove;

  /// Group settings › Invite: small heading over the people who used an invite link and wait to be let in.
  ///
  /// In en, this message translates to:
  /// **'Asking to join'**
  String get groupSettingsAskingToJoin;

  /// Group settings › Invite: small heading over the list of friends to invite.
  ///
  /// In en, this message translates to:
  /// **'Invite a friend'**
  String get groupSettingsInviteFriend;

  /// Group settings › Invite: note under the list of friends to invite.
  ///
  /// In en, this message translates to:
  /// **'They get an invitation in Fogata and choose whether to join. Only your friends show here.'**
  String get groupSettingsInviteFriendNote;

  /// Group settings › Invite: small heading over the invite link part.
  ///
  /// In en, this message translates to:
  /// **'Invite link'**
  String get groupSettingsInviteLink;

  /// Group settings › Invite: what an invite link is for and how it works. {who} says who lets the person in: anyone (any member), first (only the first member) or other (one of the first {n} members, {n} is 2 or more).
  ///
  /// In en, this message translates to:
  /// **'For someone who is not your friend yet. It works once and for 24 hours, and {who, select, anyone{someone in the group} first{the first in the group} other{one of the first {n} in the group}} lets them in: if it reaches someone else, you see who asks and say no.'**
  String groupSettingsInviteLinkNote(String who, int n);

  /// Group settings › Invite: button that makes a new invite link and copies it.
  ///
  /// In en, this message translates to:
  /// **'Copy invite link'**
  String get groupSettingsCopyLink;

  /// Group settings › Invite: how many invite links were made and not used yet.
  ///
  /// In en, this message translates to:
  /// **'{n, plural, =0{No links waiting to be used.} =1{1 link not used yet.} other{{n} links not used yet.}}'**
  String groupSettingsOpenLinks(int n);

  /// Group settings › Invite: small button that makes every unused invite link stop working. Short.
  ///
  /// In en, this message translates to:
  /// **'Cancel them'**
  String get groupSettingsCancelLinks;

  /// Invite friends list: shown when the person has no friends ("Friends" is the Friends page).
  ///
  /// In en, this message translates to:
  /// **'No friends yet: add them in Friends.'**
  String get groupSettingsNoFriends;

  /// Invite friends list: shown when every friend is already in the group.
  ///
  /// In en, this message translates to:
  /// **'All your friends are in the group.'**
  String get groupSettingsAllFriendsIn;

  /// Invite friends list: under a friend's name, they are online. Short.
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get groupSettingsFriendOnline;

  /// Invite friends list: under a friend's name, they are offline. Short.
  ///
  /// In en, this message translates to:
  /// **'Not online now'**
  String get groupSettingsFriendOffline;

  /// Invite friends list: small button on a friend's row that sends them an invitation to the group. Short.
  ///
  /// In en, this message translates to:
  /// **'Invite'**
  String get groupSettingsInviteButton;

  /// Invite friends list: tooltip of the (disabled) Invite button when the group has as many members as it takes.
  ///
  /// In en, this message translates to:
  /// **'The group is full'**
  String get groupSettingsGroupFull;

  /// Group settings › Invite › Asking to join: button that refuses someone who used an invite link. Short.
  ///
  /// In en, this message translates to:
  /// **'Say no'**
  String get groupSettingsSayNo;

  /// Group settings › Invite › Asking to join: button that lets someone who used an invite link into the group. Short.
  ///
  /// In en, this message translates to:
  /// **'Let in'**
  String get groupSettingsLetIn;

  /// Group settings › Limits: title of the slider for the most members the group takes.
  ///
  /// In en, this message translates to:
  /// **'People in the group'**
  String get groupSettingsLimitGroup;

  /// Group settings › Limits: note under the group size slider. {n} is how many members the group has now.
  ///
  /// In en, this message translates to:
  /// **'From 2 to 20. When it is full, invitations and links stop working. It cannot go below the {n, plural, =1{1 person} other{{n} people}} already in.'**
  String groupSettingsLimitGroupNote(int n);

  /// Group settings › Limits: title of the slider for the most people each voice room takes.
  ///
  /// In en, this message translates to:
  /// **'People in each voice room'**
  String get groupSettingsLimitRoom;

  /// Group settings › Limits: note under the room size slider (calls go directly between everyone, with no server).
  ///
  /// In en, this message translates to:
  /// **'From 2 to 8. Voice and video go straight between everyone in a room, so 8 is the most that sounds good; with cameras on, fewer work better. Whoever comes when it is full sees that the room is full.'**
  String get groupSettingsLimitRoomNote;

  /// Title bar chip with a lock, during a direct call of more than two people: the call goes straight between the computers, encrypted, and only those in it hear it. Short words joined by " · ".
  ///
  /// In en, this message translates to:
  /// **'Direct · encrypted · only those in the call'**
  String get shellChipCallMany;

  /// Title bar chip with a lock, during a direct call of two people: the call goes straight between the two computers, encrypted, and only the two hear it. Short words joined by " · ".
  ///
  /// In en, this message translates to:
  /// **'Direct · encrypted · only you two'**
  String get shellChipCallTwo;

  /// Title bar: tooltip of the group's name in a room, which opens the group's members.
  ///
  /// In en, this message translates to:
  /// **'Group members'**
  String get shellGroupMembers;

  /// Title bar: name of the page with a group's members, when the group's name is not known.
  ///
  /// In en, this message translates to:
  /// **'Members'**
  String get shellMembers;

  /// Title bar: name of a direct chat page (one-to-one messages with a friend), when the friend's name is not known.
  ///
  /// In en, this message translates to:
  /// **'Direct messages'**
  String get shellDirectMessages;

  /// Title bar chip (and tooltip of the lock in the person's pill) in a room: everyone is reached straight, encrypted. Short words joined by " · ".
  ///
  /// In en, this message translates to:
  /// **'Direct · encrypted'**
  String get shellPathDirect;

  /// Title bar chip in a room: some people are reached through the member {name}, who passes on the encrypted traffic (and cannot read it). Short words joined by " · ".
  ///
  /// In en, this message translates to:
  /// **'Via {name} · encrypted'**
  String shellPathVia(String name);

  /// Title bar chip in a room: some people are reached through other members, who pass on the encrypted traffic (and cannot read it). Short words joined by " · ".
  ///
  /// In en, this message translates to:
  /// **'Via a member · encrypted'**
  String get shellPathViaSomeone;

  /// Title of a direct call (title bar and side column). {names} is one name or a list of names like "Marta and Leo".
  ///
  /// In en, this message translates to:
  /// **'Call with {names}'**
  String shellCallWith(String names);

  /// Title bar pill while a direct call rings; the time it has rung (like 0:12) follows right after. Keep the " · " and the space at the end.
  ///
  /// In en, this message translates to:
  /// **'Ringing · '**
  String get shellRinging;

  /// Title bar pill during a direct call; how long it has lasted (like 12:04) follows right after. Keep the " · " and the space at the end.
  ///
  /// In en, this message translates to:
  /// **'In call · '**
  String get shellInCall;

  /// Title bar pill for a moment after a direct call ended. Short.
  ///
  /// In en, this message translates to:
  /// **'Call ended'**
  String get shellCallEnded;

  /// Rail on the left: tooltip of the "+" button at the end of the groups, which makes a new group.
  ///
  /// In en, this message translates to:
  /// **'New group'**
  String get shellNewGroup;

  /// Side column: small heading (shown in capitals) over the direct call under way.
  ///
  /// In en, this message translates to:
  /// **'Call'**
  String get shellSectionCall;

  /// Side column: a direct call to or from {name} that is still ringing. Lower case after the name.
  ///
  /// In en, this message translates to:
  /// **'{name} · ringing'**
  String shellCallRinging(String name);

  /// Side column on Home: small heading (shown in capitals) over the groups whose voice room has people in it now.
  ///
  /// In en, this message translates to:
  /// **'Live now'**
  String get shellSectionLiveNow;

  /// Side column during a direct call: small heading (shown in capitals) over the people in the call.
  ///
  /// In en, this message translates to:
  /// **'In the call'**
  String get shellSectionInCall;

  /// Side column during a direct call: item that shows the call's chat. Short.
  ///
  /// In en, this message translates to:
  /// **'Chat'**
  String get shellChat;

  /// Side column during a direct call: item that shows the files shared in the call. Short.
  ///
  /// In en, this message translates to:
  /// **'Files'**
  String get shellFiles;

  /// Side column of a group: small heading (shown in capitals) over the group's own items (members, invite).
  ///
  /// In en, this message translates to:
  /// **'Group'**
  String get shellSectionGroup;

  /// Side column of a group: item that opens the group's invitations (friends and invite links). Short.
  ///
  /// In en, this message translates to:
  /// **'Invite'**
  String get shellInvite;

  /// Side column of a group: small heading (shown in capitals) over the group's voice rooms.
  ///
  /// In en, this message translates to:
  /// **'Voice rooms'**
  String get shellSectionVoiceRooms;

  /// Side column of a group: the voice room {room} this person is in. Lower case after the name. Where the column is narrow the room's name is cut short and the words around it stay whole, so keep them short.
  ///
  /// In en, this message translates to:
  /// **'{room} · you are in'**
  String shellVoiceRoomYouAreIn(String room);

  /// Side column of a group: shown instead of the count on a voice room this person may not enter (only some members can). Short.
  ///
  /// In en, this message translates to:
  /// **'Not for you now'**
  String get shellNotForYou;

  /// Side column: screen-reader label of the small lock on a voice room that only some members can enter.
  ///
  /// In en, this message translates to:
  /// **'Only some members'**
  String get shellOnlySomeMembers;

  /// Side column: this person's own name in the list of people in a voice room or call.
  ///
  /// In en, this message translates to:
  /// **'{name} (you)'**
  String shellPersonYou(String name);

  /// Side column: tiny red label beside someone who shares their screen, shown in capitals. Short: one or two short words; the label grows with it and the name beside it is cut instead.
  ///
  /// In en, this message translates to:
  /// **'LIVE'**
  String get shellLive;

  /// The person's own pill at the bottom left: screen-reader label; clicking it opens their profile panel.
  ///
  /// In en, this message translates to:
  /// **'Your profile'**
  String get shellYourProfile;

  /// The person's pill: tooltip of the microphone button while it is on, with the keyboard shortcut {keys} (like Ctrl+Shift+M).
  ///
  /// In en, this message translates to:
  /// **'Mute microphone ({keys})'**
  String shellMuteMic(String keys);

  /// The person's pill: tooltip of the microphone button while it is muted, with the keyboard shortcut {keys} (like Ctrl+Shift+M).
  ///
  /// In en, this message translates to:
  /// **'Unmute microphone ({keys})'**
  String shellUnmuteMic(String keys);

  /// The person's pill: tooltip of the headphones button: stop hearing everyone (and mute the microphone). Same words as in the room's controls.
  ///
  /// In en, this message translates to:
  /// **'Stop hearing the others'**
  String get shellDeafen;

  /// The person's pill: tooltip of the headphones button while deafened.
  ///
  /// In en, this message translates to:
  /// **'Hear the others again'**
  String get shellUndeafen;

  /// The person's pill at the bottom left, under their name: what they are doing. Very short (the pill is narrow). A direct call rings; the friend's name follows after " · ".
  ///
  /// In en, this message translates to:
  /// **'Calling…'**
  String get shellStateCalling;

  /// The person's pill at the bottom left, under their name: what they are doing. Very short (the pill is narrow). In a direct call; the friend's name follows after " · ".
  ///
  /// In en, this message translates to:
  /// **'In a call'**
  String get shellStateInCall;

  /// The person's pill at the bottom left, under their name: what they are doing. Very short (the pill is narrow). In a group's voice room.
  ///
  /// In en, this message translates to:
  /// **'In the room'**
  String get shellStateInRoom;

  /// The person's pill at the bottom left, under their name: what they are doing. Very short (the pill is narrow). Appearing offline to others.
  ///
  /// In en, this message translates to:
  /// **'Offline'**
  String get shellStateOffline;

  /// The person's pill at the bottom left, under their name: what they are doing. Very short (the pill is narrow).
  ///
  /// In en, this message translates to:
  /// **'Online'**
  String get shellStateOnline;

  /// The person's pill at the bottom left, under their name: friends and members of their groups see them as away, because nobody used this computer for 10 minutes. Very short (the pill is narrow).
  ///
  /// In en, this message translates to:
  /// **'Away'**
  String get shellStateAway;

  /// Toast about a saved file: small button that opens the folder with the file selected. Short.
  ///
  /// In en, this message translates to:
  /// **'Show in folder'**
  String get shellShowInFolder;

  /// Toast about a saved file: small button that copies where the file is on the computer. Short.
  ///
  /// In en, this message translates to:
  /// **'Copy path'**
  String get shellCopyPath;

  /// Side column: a person's voice volume in percent, beside their name when it is not 100 (like 150%). Write the percent the way the language does (for example "150 %").
  ///
  /// In en, this message translates to:
  /// **'{n}%'**
  String shellVolumePercent(int n);

  /// The keyboard shortcut that mutes the microphone on Windows and Linux, written as keyboards in the language label the keys (German keyboards say Strg+Umschalt+M, for example). Only the key names change; the letter stays M.
  ///
  /// In en, this message translates to:
  /// **'Ctrl+Shift+M'**
  String get shellMuteKeys;

  /// Settings: name of the part that asks for a donation to Fogata (in the settings list, under User, with a heart). Short.
  ///
  /// In en, this message translates to:
  /// **'Donate to Fogata'**
  String get settingsSectionDonate;

  /// Settings: name of the part where the person describes a problem and sends it to the Fogata team (in the settings list under Help, and as the part's title). Short.
  ///
  /// In en, this message translates to:
  /// **'Report a problem'**
  String get settingsSectionReport;

  /// Settings list: small heading (shown in capitals) above Report a problem, at the end of the list.
  ///
  /// In en, this message translates to:
  /// **'Help'**
  String get settingsHeadingHelp;

  /// Settings: small red label (shown in capitals) beside the version at the bottom of the settings list: this copy is an early test version, before the first full release. Very short.
  ///
  /// In en, this message translates to:
  /// **'Pre-release'**
  String get settingsPreRelease;

  /// Settings › Donate to Fogata: the title under the person's picture.
  ///
  /// In en, this message translates to:
  /// **'Keep Fogata independent'**
  String get donateTitle;

  /// Settings › Donate to Fogata: the line under the title.
  ///
  /// In en, this message translates to:
  /// **'Fogata is free, has no ads and no servers watching you. Your donation keeps it that way.'**
  String get donateText;

  /// Settings › Donate to Fogata: the main button, with a heart; opens the donation page in the browser. Very short.
  ///
  /// In en, this message translates to:
  /// **'Donate'**
  String get donateButton;

  /// Settings › Donate to Fogata: small line under the button. Ko-fi is the name of the donation site.
  ///
  /// In en, this message translates to:
  /// **'Opens Ko-fi in your browser. Fogata never sees your payment.'**
  String get donateNote;

  /// Settings › Report a problem: title of the box where the person writes what went wrong; also the box's label for screen readers.
  ///
  /// In en, this message translates to:
  /// **'What happened?'**
  String get reportWhatTitle;

  /// Settings › Report a problem: line under "What happened?", saying what to write.
  ///
  /// In en, this message translates to:
  /// **'What you were doing, what you expected and what you saw instead.'**
  String get reportWhatNote;

  /// Settings › Report a problem: checkbox, ticked at first: the report also carries the app's technical log of the last two days.
  ///
  /// In en, this message translates to:
  /// **'Include the technical log of the last 2 days'**
  String get reportIncludeLog;

  /// Settings › Report a problem: small line under the log checkbox, followed by the link "See what’s sent".
  ///
  /// In en, this message translates to:
  /// **'Versions, errors and connection steps. Never your messages, keys, contacts or IP addresses.'**
  String get reportLogNote;

  /// Settings › Report a problem: underlined link after the log note; shows the list of everything the report holds.
  ///
  /// In en, this message translates to:
  /// **'See what’s sent'**
  String get reportSeeSent;

  /// Settings › Report a problem: the same link once the list shows; hides it again.
  ///
  /// In en, this message translates to:
  /// **'Hide the list'**
  String get reportHideSent;

  /// Settings › Report a problem: underlined link under the log checkbox; shows, in a box that scrolls, the whole report exactly as it will be sent.
  ///
  /// In en, this message translates to:
  /// **'Read the whole report before sending'**
  String get reportPreview;

  /// Settings › Report a problem: the same link once the report shows; hides it again.
  ///
  /// In en, this message translates to:
  /// **'Hide the report'**
  String get reportHidePreview;

  /// Settings › Report a problem: small line above the box that shows the whole report as it will be sent.
  ///
  /// In en, this message translates to:
  /// **'This is exactly what the file holds, and all that goes. Names, folders, file names, addresses and keys are already taken out of the log.'**
  String get reportPreviewNote;

  /// Settings › Report a problem: shown in place of the report for the moment it takes to make it.
  ///
  /// In en, this message translates to:
  /// **'Preparing the report…'**
  String get reportPreviewWorking;

  /// Settings › Report a problem › See what’s sent: first item of the list of what the report holds.
  ///
  /// In en, this message translates to:
  /// **'What you wrote above.'**
  String get reportSentWhat;

  /// Settings › Report a problem › See what’s sent: second item. "Build" is the number of this release of the app.
  ///
  /// In en, this message translates to:
  /// **'Fogata’s version and build, your system and its version, and the language Fogata is in.'**
  String get reportSentVersion;

  /// Settings › Report a problem › See what’s sent: third item, shown while the log checkbox is ticked: what the log holds and what is taken out of it.
  ///
  /// In en, this message translates to:
  /// **'The log of the last 2 days: when Fogata started, its errors and how it reached other devices. Names, messages, folders, file names, links, codes, keys and IP addresses are taken out, and each device shows only as a number that means nothing outside the report.'**
  String get reportSentLog;

  /// Settings › Report a problem: main button (paper plane icon): saves the report file, opens the mail app with the message ready and the folder with the file.
  ///
  /// In en, this message translates to:
  /// **'Email it to the Fogata team'**
  String get reportEmail;

  /// Settings › Report a problem: second button (file icon): saves the report file where the person chooses.
  ///
  /// In en, this message translates to:
  /// **'Save to a file'**
  String get reportSave;

  /// Settings › Report a problem: note under the box, saying what "Email it" does. {address} is the team's e-mail address, drawn in bold.
  ///
  /// In en, this message translates to:
  /// **'Your email app opens with the message ready for {address}, and the folder with the report file opens beside it: attach the file and send. Nothing leaves your computer until you do.'**
  String reportHowEmail(String address);

  /// Settings › Report a problem, after "Email it": title over a green check.
  ///
  /// In en, this message translates to:
  /// **'Your email is ready'**
  String get reportReadyTitle;

  /// Settings › Report a problem, after "Email it": line under the title. {file} is the report's file name (drawn in a monospace font), {address} the team's e-mail address (bold).
  ///
  /// In en, this message translates to:
  /// **'Attach {file} from the folder that opened and send it to {address}.'**
  String reportReadyText(String file, String address);

  /// Settings › Report a problem, after "Email it": button (folder icon) that shows the report file in its folder.
  ///
  /// In en, this message translates to:
  /// **'Show the file'**
  String get reportShowFile;

  /// Settings › Report a problem, after "Email it": button that copies the team's e-mail address.
  ///
  /// In en, this message translates to:
  /// **'Copy the address'**
  String get reportCopyAddress;

  /// Toast after the team's e-mail address was copied.
  ///
  /// In en, this message translates to:
  /// **'Address copied'**
  String get reportAddressCopied;

  /// Toast after the report was saved with "Save to a file". {file} is the file's name.
  ///
  /// In en, this message translates to:
  /// **'Report saved: {file}'**
  String reportSaved(String file);

  /// Toast when no mail application opened for "Email it". {address} is the team's e-mail address.
  ///
  /// In en, this message translates to:
  /// **'No email app opened. Send the file to {address} from the email you use.'**
  String reportNoMailApp(String address);

  /// Toast when the report file could not be written. {reason} is the system's error.
  ///
  /// In en, this message translates to:
  /// **'The report could not be written: {reason}'**
  String reportFailed(String reason);

  /// Small window shown while Fogata starts: what it is doing (shown in capitals, italic monospace, under the logo). Fogata is opening. Very short: the window is 300 px wide.
  ///
  /// In en, this message translates to:
  /// **'Starting'**
  String get splashStarting;

  /// Start window: Fogata is connecting to the network. Very short (300 px wide window, shown in capitals).
  ///
  /// In en, this message translates to:
  /// **'Connecting'**
  String get splashConnecting;

  /// Start window: Fogata asks friends for a newer version. Very short (300 px wide window, shown in capitals).
  ///
  /// In en, this message translates to:
  /// **'Checking for updates'**
  String get splashChecking;

  /// Start window: a newer version is downloading from a friend; a bar and the amount show below. Very short (300 px wide window, shown in capitals).
  ///
  /// In en, this message translates to:
  /// **'Downloading update'**
  String get splashDownloading;

  /// Start window: the new version is being installed. Very short (300 px wide window, shown in capitals).
  ///
  /// In en, this message translates to:
  /// **'Installing update'**
  String get splashInstalling;

  /// Start window: Fogata closes and opens again as the new version. Very short (300 px wide window, shown in capitals).
  ///
  /// In en, this message translates to:
  /// **'Restarting'**
  String get splashRestarting;

  /// Start window, under the download bar: how much of the update is here. {done} and {total} are numbers of megabytes, already formatted ("34", "58"). Short.
  ///
  /// In en, this message translates to:
  /// **'{done} of {total} MB'**
  String splashProgress(String done, String total);

  /// Start window, under the download bar: how much of the update is here and from whose computer it comes. {done} and {total} are numbers of megabytes ("34", "58"); {name} is a friend's name. Short.
  ///
  /// In en, this message translates to:
  /// **'{done} of {total} MB · from {name}'**
  String splashProgressFrom(String done, String total, String name);

  /// Green pill in the title bar while Fogata runs: a newer version was downloaded and checked; a Restart button follows. {version} is like 0.1.1. Short.
  ///
  /// In en, this message translates to:
  /// **'Update ready · {version}'**
  String updateReady(String version);

  /// Amber pill in the title bar: a newer version is ready and this one no longer works with it; a Restart button follows. {version} is like 0.1.1. Short.
  ///
  /// In en, this message translates to:
  /// **'Update required · {version}'**
  String updateRequired(String version);

  /// Small button in the update pill of the title bar: installs the new version and opens Fogata again. Very short.
  ///
  /// In en, this message translates to:
  /// **'Restart'**
  String get updateRestart;

  /// Tooltip of the green update pill in the title bar. {version} is like 0.1.1.
  ///
  /// In en, this message translates to:
  /// **'Fogata {version} is here. Restart to use it.'**
  String updateReadyTip(String version);

  /// Tooltip of the amber update pill in the title bar.
  ///
  /// In en, this message translates to:
  /// **'This version no longer works with your friends’ newer one. Restart to update.'**
  String get updateRequiredTip;

  /// Toast when the new version could not be installed. {reason} says why (may be in English).
  ///
  /// In en, this message translates to:
  /// **'The update could not be installed: {reason}'**
  String updateFailed(String reason);

  /// Error message shown to a manager who let someone into a group with an invite: the invite no longer works, or the group has no room left.
  ///
  /// In en, this message translates to:
  /// **'That invite no longer works, or the group is full.'**
  String get errJoinInviteGone;

  /// Error message shown to a manager who let someone into a group: this device has not received the group's encryption key yet, and the person gets in as soon as it has.
  ///
  /// In en, this message translates to:
  /// **'This device doesn’t have the group’s key yet. The new member gets in as soon as it does.'**
  String get errJoinNoKeyYet;

  /// Error message shown to a manager who let someone into a group: this device made changes the other members have not taken in yet, so the person can be let in once another member is online.
  ///
  /// In en, this message translates to:
  /// **'This device has made changes the other members haven’t taken in yet. The new member can be let in once another member is online.'**
  String get errJoinWroteTooMuch;

  /// Error message shown to a manager who let someone into a group: the person's request was made on an earlier state of the group, so they have to ask to join again.
  ///
  /// In en, this message translates to:
  /// **'That request to join was made on an earlier state of the group. The person has to ask again.'**
  String get errJoinAskAgain;

  /// Error message when downloading a room's file: the disk does not have enough free space for it.
  ///
  /// In en, this message translates to:
  /// **'There isn’t enough free space for this file. Free some space and try again.'**
  String get errNoSpaceForFile;

  /// Settings › Storage › Recovery copy: title of the section.
  ///
  /// In en, this message translates to:
  /// **'Recovery copy'**
  String get recoveryTitle;

  /// Settings › Storage › Recovery copy: note under the title.
  ///
  /// In en, this message translates to:
  /// **'A file that brings this account back if this computer’s keychain or disk is lost. Only your recovery key opens it.'**
  String get recoveryNote;

  /// Settings › Storage › Recovery copy: status on the right of the title: copies are off.
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get recoveryOff;

  /// Settings › Storage › Recovery copy: status on the right of the title: copies are on. {id} is the recovery key's short name, like "7Q4M K2XR".
  ///
  /// In en, this message translates to:
  /// **'On · key {id}'**
  String recoveryOn(String id);

  /// Settings › Storage › Recovery copy: button that turns copies on (shows a new recovery key).
  ///
  /// In en, this message translates to:
  /// **'Turn on…'**
  String get recoveryTurnOn;

  /// Settings › Storage › Recovery copy: button that makes a new recovery key while copies are on.
  ///
  /// In en, this message translates to:
  /// **'Change key…'**
  String get recoveryChangeKey;

  /// Settings › Storage › Recovery copy: button that makes a copy of the account in a file.
  ///
  /// In en, this message translates to:
  /// **'Make a copy…'**
  String get recoveryMakeCopy;

  /// Settings › Storage › Recovery copy: line shown when restoring a copy replaced an account, which is kept aside.
  ///
  /// In en, this message translates to:
  /// **'An account that a restore replaced is still kept on this computer.'**
  String get recoveryReplacedKept;

  /// Settings › Storage › Recovery copy: button beside that line: deletes the account kept aside (asks first).
  ///
  /// In en, this message translates to:
  /// **'Delete it…'**
  String get recoveryDeleteReplaced;

  /// Settings › Storage › Recovery copy: title of the dialog before changing the key.
  ///
  /// In en, this message translates to:
  /// **'Make a new recovery key?'**
  String get recoveryChangeTitle;

  /// Settings › Storage › Recovery copy: text of that dialog.
  ///
  /// In en, this message translates to:
  /// **'New copies will open only with the new key. Copies made before still open only with the old one, so keep it as long as you keep them.'**
  String get recoveryChangeText;

  /// Settings › Storage › Recovery copy: button of that dialog that goes on.
  ///
  /// In en, this message translates to:
  /// **'Make a new key'**
  String get recoveryChangeConfirm;

  /// Settings › Storage › Recovery copy: title of the dialog that shows the new key once.
  ///
  /// In en, this message translates to:
  /// **'Your recovery key'**
  String get recoveryKeyTitle;

  /// Settings › Storage › Recovery copy: text above the key in that dialog.
  ///
  /// In en, this message translates to:
  /// **'Write it down or print it, and keep it apart from your copies. It is shown only this once.'**
  String get recoveryKeyText;

  /// Settings › Storage › Recovery copy: under the key: its short name, to compare later. {id} is like "7Q4M K2XR".
  ///
  /// In en, this message translates to:
  /// **'Its short name: {id}'**
  String recoveryKeyName(String id);

  /// Settings › Storage › Recovery copy: warning under the key.
  ///
  /// In en, this message translates to:
  /// **'Fogata doesn’t keep this key anywhere: without it, no copy opens. Anyone with a copy and this key has your account.'**
  String get recoveryKeyWarning;

  /// Settings › Storage › Recovery copy: button under the key: goes on to check two groups of it.
  ///
  /// In en, this message translates to:
  /// **'I wrote it down'**
  String get recoveryKeyWritten;

  /// Settings › Storage › Recovery copy: title of the step that asks two groups of the key back.
  ///
  /// In en, this message translates to:
  /// **'Check what you wrote down'**
  String get recoveryCheckTitle;

  /// Settings › Storage › Recovery copy: text of that step.
  ///
  /// In en, this message translates to:
  /// **'Type these two groups of the key, as you wrote them.'**
  String get recoveryCheckText;

  /// Settings › Storage › Recovery copy: label over a field of that step: which group of the key to type, counted from 1.
  ///
  /// In en, this message translates to:
  /// **'Group {n}'**
  String recoveryCheckGroup(int n);

  /// Settings › Storage › Recovery copy: error when a group typed is not the key's.
  ///
  /// In en, this message translates to:
  /// **'That isn’t it: look again at what you wrote down.'**
  String get recoveryCheckWrong;

  /// Settings › Storage › Recovery copy: button that takes the key once both groups are right.
  ///
  /// In en, this message translates to:
  /// **'Turn on copies'**
  String get recoveryTurnOnButton;

  /// Settings › Storage › Recovery copy: toast once copies are on. {id} is the key's short name.
  ///
  /// In en, this message translates to:
  /// **'Copies are on, for the key {id}.'**
  String recoveryTurnedOn(String id);

  /// Settings › Storage › Recovery copy: title of the dialog before making a copy.
  ///
  /// In en, this message translates to:
  /// **'Make a recovery copy'**
  String get recoveryCopyTitle;

  /// Settings › Storage › Recovery copy: text of that dialog.
  ///
  /// In en, this message translates to:
  /// **'Fogata is offline while it makes the copy, longer with the files sent in chats. Keep the file away from this computer, on a USB drive for example.'**
  String get recoveryCopyText;

  /// Settings › Storage › Recovery copy: checkbox in that dialog, off at first.
  ///
  /// In en, this message translates to:
  /// **'Include files sent in chats'**
  String get recoveryCopyAttachments;

  /// Settings › Storage › Recovery copy: under that checkbox: what they add to the copy. {size} is like "48.2 MB".
  ///
  /// In en, this message translates to:
  /// **'Pictures, videos and files: {size} more.'**
  String recoveryCopyAttachmentsNote(String size);

  /// Settings › Storage › Recovery copy: button of that dialog that opens the system's save dialog.
  ///
  /// In en, this message translates to:
  /// **'Choose where…'**
  String get recoveryCopySave;

  /// Settings › Storage › Recovery copy: error when the place chosen is in Fogata's own data folder.
  ///
  /// In en, this message translates to:
  /// **'A copy can’t be kept in Fogata’s data folder. Choose another place.'**
  String get recoveryCopyInDataFolder;

  /// Settings › Storage › Recovery copy: toast once the copy is made. {file} is the file's name, {size} like "48.2 MB".
  ///
  /// In en, this message translates to:
  /// **'Copy saved: {file} ({size}).'**
  String recoveryCopyMade(String file, String size);

  /// Settings › Storage › Recovery copy: toast when making the copy failed. {why} is the reason, a sentence.
  ///
  /// In en, this message translates to:
  /// **'The copy couldn’t be made. {why}'**
  String recoveryCopyFailed(String why);

  /// Settings › Storage › Recovery copy: title of the dialog before deleting the account a restore replaced.
  ///
  /// In en, this message translates to:
  /// **'Delete the replaced account?'**
  String get recoveryDeleteTitle;

  /// Settings › Storage › Recovery copy: text of that dialog.
  ///
  /// In en, this message translates to:
  /// **'It was set aside when a copy was restored over it. Once deleted, it can’t be brought back.'**
  String get recoveryDeleteText;

  /// Settings › Storage › Recovery copy: red button of that dialog.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get recoveryDelete;

  /// Settings › Storage › Recovery copy: toast once it is deleted.
  ///
  /// In en, this message translates to:
  /// **'The replaced account was deleted.'**
  String get recoveryDeleted;

  /// Name of the kind of file a recovery copy is, in the system's open and save dialogs.
  ///
  /// In en, this message translates to:
  /// **'Fogata copy'**
  String get recoveryFileType;

  /// Welcome page: first words of the link under the profile card, before "Restore from a copy".
  ///
  /// In en, this message translates to:
  /// **'Lost your computer?'**
  String get recoveryLostComputer;

  /// Welcome page: the link under the profile card that restores the account from a recovery copy.
  ///
  /// In en, this message translates to:
  /// **'Restore from a copy'**
  String get recoveryRestoreLink;

  /// Welcome page › Restore from a copy: title of the card.
  ///
  /// In en, this message translates to:
  /// **'Restore from a copy'**
  String get recoveryRestoreTitle;

  /// Welcome page › Restore from a copy: text under the title before a file is chosen.
  ///
  /// In en, this message translates to:
  /// **'Choose the recovery copy you saved, then type your recovery key.'**
  String get recoveryRestoreSub;

  /// Welcome page › Restore from a copy: button that opens the system's dialog to pick the file.
  ///
  /// In en, this message translates to:
  /// **'Choose the copy…'**
  String get recoveryChooseCopy;

  /// Welcome page › Restore from a copy: under the title once a file is chosen. {date} is a day with its year, like "5 Nov 2026".
  ///
  /// In en, this message translates to:
  /// **'A copy made on {date}.'**
  String recoveryMadeOn(String date);

  /// Welcome page › Restore from a copy: note when the copy is for another key than this computer's.
  ///
  /// In en, this message translates to:
  /// **'It was made for a different recovery key from the one this computer has. Type the key it was made for.'**
  String get recoveryOtherKey;

  /// Welcome page › Restore from a copy: screen-reader label of the field where the key is typed.
  ///
  /// In en, this message translates to:
  /// **'Recovery key'**
  String get recoveryKeyField;

  /// Welcome page › Restore from a copy: error while typing the key: it is not shaped like one.
  ///
  /// In en, this message translates to:
  /// **'A recovery key has 53 letters and digits, in groups of four.'**
  String get recoveryKeyNotAKey;

  /// Welcome page › Restore from a copy: error while typing the key: a letter or digit is wrong.
  ///
  /// In en, this message translates to:
  /// **'A letter or digit is wrong: check them.'**
  String get recoveryKeyMistyped;

  /// Welcome page › Restore from a copy: error: the file chosen is not a copy.
  ///
  /// In en, this message translates to:
  /// **'This isn’t a copy of a Fogata account.'**
  String get recoveryProblemNotACopy;

  /// Welcome page › Restore from a copy: error: the copy is from a newer version.
  ///
  /// In en, this message translates to:
  /// **'This copy was made by a newer version of Fogata. Update Fogata to open it.'**
  String get recoveryProblemNewer;

  /// Welcome page › Restore from a copy: error: the key typed is not the copy's.
  ///
  /// In en, this message translates to:
  /// **'This copy was made for another recovery key.'**
  String get recoveryProblemOtherKey;

  /// Welcome page › Restore from a copy: error: the copy does not read whole.
  ///
  /// In en, this message translates to:
  /// **'This copy is damaged, or was changed after it was made.'**
  String get recoveryProblemDamaged;

  /// Welcome page › Restore from a copy: error: the copy opens with the key but was not made on a computer that had the account.
  ///
  /// In en, this message translates to:
  /// **'This copy wasn’t made by the account it holds, so it won’t be opened.'**
  String get recoveryProblemUnproven;

  /// Welcome page › Restore from a copy: title once the key opened the copy. {name} is the account's name.
  ///
  /// In en, this message translates to:
  /// **'{name}’s account'**
  String recoveryWhose(String name);

  /// Welcome page › Restore from a copy: under that title: what the copy holds. {size} is like "48.2 MB".
  ///
  /// In en, this message translates to:
  /// **'{files, plural, =1{1 file} other{{files} files}}, {size} in all'**
  String recoveryHolds(int files, String size);

  /// Welcome page › Restore from a copy: under that: the copy has the files sent in chats.
  ///
  /// In en, this message translates to:
  /// **'With the files sent in chats.'**
  String get recoveryWithAttachments;

  /// Welcome page › Restore from a copy: under that: the copy has not the files sent in chats.
  ///
  /// In en, this message translates to:
  /// **'Without the files sent in chats.'**
  String get recoveryWithoutAttachments;

  /// Welcome page › Restore from a copy: warning before restoring.
  ///
  /// In en, this message translates to:
  /// **'Use a copy only if no other computer uses this account any more. If one still does, move the account from it instead: one account can’t be on two computers.'**
  String get recoveryOnlyIf;

  /// Welcome page › Restore from a copy: button under that warning: goes to bringing the account from another computer.
  ///
  /// In en, this message translates to:
  /// **'Move it instead'**
  String get recoveryMoveInstead;

  /// Welcome page › Restore from a copy: what this computer holds now, which restoring replaces. {name} is that account's name.
  ///
  /// In en, this message translates to:
  /// **'This computer holds {name}’s account.'**
  String recoveryHereNamed(String name);

  /// Welcome page › Restore from a copy: what this computer holds now: a device key without a profile. {fingerprint} is like "7Q4M K2XR 9PLD 3VHC".
  ///
  /// In en, this message translates to:
  /// **'This computer holds an account with no profile (device {fingerprint}).'**
  String recoveryHereDevice(String fingerprint);

  /// Welcome page › Restore from a copy: what this computer holds now: something of an account that does not open.
  ///
  /// In en, this message translates to:
  /// **'This computer holds part of an account that doesn’t open.'**
  String get recoveryHereUnknown;

  /// Welcome page › Restore from a copy: under that line. Name the settings as they are named in the language.
  ///
  /// In en, this message translates to:
  /// **'Restoring replaces it. It is set aside, not deleted, until you delete it in Settings › Storage & downloads.'**
  String get recoveryHereReplaced;

  /// Welcome page › Restore from a copy: button that restores the copy.
  ///
  /// In en, this message translates to:
  /// **'Restore'**
  String get recoveryRestore;

  /// Welcome page › Restore from a copy: that button when it replaces what this computer holds.
  ///
  /// In en, this message translates to:
  /// **'Replace and restore'**
  String get recoveryReplace;

  /// Welcome page › Restore from a copy: while restoring. {name} is the account's name.
  ///
  /// In en, this message translates to:
  /// **'Restoring {name}’s account…'**
  String recoveryRestoring(String name);

  /// Welcome page › Restore from a copy: toast once restored and Fogata started with it.
  ///
  /// In en, this message translates to:
  /// **'{name}’s account is restored.'**
  String recoveryRestored(String name);

  /// Welcome page › Restore from a copy: error: an account appeared on this computer meanwhile; nothing changed.
  ///
  /// In en, this message translates to:
  /// **'This computer holds an account now. Choose the copy again to see what it replaces.'**
  String get recoveryRefusedAccountHere;

  /// Welcome page › Restore from a copy: error; nothing changed. Name the settings as they are named in the language.
  ///
  /// In en, this message translates to:
  /// **'This computer still keeps an account that an earlier restore replaced. Delete it to restore this copy.'**
  String get recoveryRefusedReplacedKept;

  /// Welcome page › Restore from a copy: error. {need} and {free} are sizes like "48.2 MB".
  ///
  /// In en, this message translates to:
  /// **'The copy needs {need} on this disk, and {free} are free. Nothing changed.'**
  String recoveryRefusedNoRoom(String need, String free);

  /// Welcome page › Restore from a copy: error; nothing changed.
  ///
  /// In en, this message translates to:
  /// **'The copy is inside Fogata’s data folder. Move it somewhere else, then choose it again.'**
  String get recoveryRefusedInFolder;

  /// Settings › Storage › Recovery copy: toast when making a copy or deleting the replaced account is asked while another such task or a move has Fogata stopped.
  ///
  /// In en, this message translates to:
  /// **'Fogata is busy with a recovery copy or a move. Try again when it’s done.'**
  String get recoveryBusy;

  /// Move account: toast when a move is started while Fogata is stopped for a recovery copy.
  ///
  /// In en, this message translates to:
  /// **'Fogata is busy with a recovery copy. Move the account once it’s done.'**
  String get moveBusyWithCopy;

  /// Welcome page › Restore from a copy: red button under the error that an earlier restore's account is still kept; deletes it (asks first), then restores.
  ///
  /// In en, this message translates to:
  /// **'Delete the account kept aside…'**
  String get recoveryDeleteKeptAside;

  /// Why someone could not be reached (friend card, a room or call that could not connect): for a while nobody at a public address answered and the dials there failed; no connection, or a firewall (a work network letting only web traffic out, say).
  ///
  /// In en, this message translates to:
  /// **'Fogata can’t reach anyone on the internet from here. Check the connection, or a firewall may be blocking Fogata.'**
  String get reachOffline;

  /// Why someone could not be reached: this computer's own firewall lets nothing in for Fogata; the notifications (the bell) show the command.
  ///
  /// In en, this message translates to:
  /// **'This computer’s firewall lets nothing in for Fogata. The command that lets it in is in the notifications.'**
  String get reachFirewall;

  /// Why someone could not be reached: peers that answered over TCP did not over QUIC, so this network or this computer's firewall does not let UDP out.
  ///
  /// In en, this message translates to:
  /// **'This network or this computer blocks UDP. Calls and direct connections through routers need it; until it is allowed, others reach you only through a group member or from a public address.'**
  String get reachUdpBlocked;

  /// Why someone could not be reached: the router changes its public port for each destination (a symmetric, or hard, NAT), so hole punching cannot work from here.
  ///
  /// In en, this message translates to:
  /// **'Your router gives a different public port to each destination (a symmetric NAT), so no direct way can be opened through it. Friends behind another router reach you only through a member of a group you share.'**
  String get reachSymmetricNat;

  /// Said after reachSymmetricNat only: there is no public IPv6 address that peers reached, which would have gone around the router.
  ///
  /// In en, this message translates to:
  /// **'There is no working IPv6 either, which would get around the router.'**
  String get reachNoIpv6;

  /// Friend card of an offline friend: nothing on this computer's side is in the way right now (an answer from the internet a moment ago, UDP works, the router can be opened), so the cause is on the friend's side: offline, or their network. {name} is the friend's name.
  ///
  /// In en, this message translates to:
  /// **'Your network looks fine for Fogata, so what keeps you apart is probably on {name}’s side (offline, or their network).'**
  String reachFineFriend(String name);

  /// A room or a call that could not connect: nothing on this computer's side is in the way right now, so the cause is likely on the others' side.
  ///
  /// In en, this message translates to:
  /// **'Your network looks fine for Fogata, so what keeps you apart is probably on the other side.'**
  String get reachFineOthers;
}

class _L10nDelegate extends LocalizationsDelegate<L10n> {
  const _L10nDelegate();

  @override
  Future<L10n> load(Locale locale) {
    return SynchronousFuture<L10n>(lookupL10n(locale));
  }

  @override
  bool isSupported(Locale locale) => <String>[
    'de',
    'en',
    'es',
    'fr',
    'it',
    'ja',
    'pt',
    'ru',
    'zh',
  ].contains(locale.languageCode);

  @override
  bool shouldReload(_L10nDelegate old) => false;
}

L10n lookupL10n(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'de':
      return L10nDe();
    case 'en':
      return L10nEn();
    case 'es':
      return L10nEs();
    case 'fr':
      return L10nFr();
    case 'it':
      return L10nIt();
    case 'ja':
      return L10nJa();
    case 'pt':
      return L10nPt();
    case 'ru':
      return L10nRu();
    case 'zh':
      return L10nZh();
  }

  throw FlutterError(
    'L10n.delegate failed to load unsupported locale "$locale". This is likely '
    'an issue with the localizations generation tool. Please file an issue '
    'on GitHub with a reproducible sample app and the gen-l10n configuration '
    'that was used.',
  );
}
