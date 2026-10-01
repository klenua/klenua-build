import React, { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/inter/latin-400.css";
import "@fontsource/inter/latin-500.css";
import "@fontsource/inter/latin-600.css";
import "@fontsource/inter/latin-700.css";
import { invoke } from "@tauri-apps/api/core";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { IconAlertCircle, IconAlertCircleFilled, IconArrowUpRight, IconBell, IconBellFilled, IconCircleCheck, IconCircleCheckFilled, IconCircleX, IconCircleXFilled, IconCloudUpload, IconDownload, IconDownloadFilled, IconFolderOpen, IconFolderOpenFilled, IconGauge, IconGaugeFilled, IconHelpCircle, IconHelpCircleFilled, IconHistory, IconInfoCircle, IconInfoCircleFilled, IconKeyboard, IconKeyboardFilled, IconLoader2, IconLock, IconLockFilled, IconLogin, IconLogout, IconRefresh, IconSettings, IconSettingsFilled, IconShieldCheck, IconTrash, IconTrashFilled, IconUserCircle } from "@tabler/icons-react";
import kleunaLogo from "./assets/klenua-logo.png";
import type { AppSettings, CheckReport, KlenuaAccount, KlenuaSyncStatus, Preview, ProjectScan, RecentProject, RenamePreview } from "./types";
import "./check-panel.css";
import "./styles.css";
import "./pages.css";
import "./branding.css";
import "./dashboard.css";
import "./auth.css";
import "./projects.css";
import "./sync-settings.css";
import "./backup-upload.css";

const api = <T,>(command: string, args?: Record<string, unknown>) => invoke<T>(command, args);
type Page = "overview" | "project" | "settings" | "help";
type Toast = { title: string; message: string; tone: "success" | "error" | "info" };
type AvailableUpdate = { version: string; notes?: string };
const bump = (value: string, type: "patch" | "minor" | "major") => {
  const parts = value.split(".").map(Number);
  if (parts.length !== 3 || parts.some(Number.isNaN)) return value;
  if (type === "major") return `${parts[0] + 1}.0.0`;
  if (type === "minor") return `${parts[0]}.${parts[1] + 1}.0`;
  return `${parts[0]}.${parts[1]}.${parts[2] + 1}`;
};

const tablerIcon = (Icon: any, FilledIcon?: any) => ({ weight, ...props }: any) => {
  const Component = weight === "fill" && FilledIcon ? FilledIcon : Icon;
  return <Component {...props} stroke={1.8} />;
};
const ArrowClockwise = tablerIcon(IconRefresh);
const ArrowUpRight = tablerIcon(IconArrowUpRight);
const Bell = tablerIcon(IconBell, IconBellFilled);
const CheckCircle = tablerIcon(IconCircleCheck, IconCircleCheckFilled);
const ClockCounterClockwise = tablerIcon(IconHistory);
const CloudArrowUp = tablerIcon(IconCloudUpload);
const DownloadSimple = tablerIcon(IconDownload, IconDownloadFilled);
const FolderOpen = tablerIcon(IconFolderOpen, IconFolderOpenFilled);
const Gauge = tablerIcon(IconGauge, IconGaugeFilled);
const Gear = tablerIcon(IconSettings, IconSettingsFilled);
const InfoCircle = tablerIcon(IconInfoCircle, IconInfoCircleFilled);
const Keyboard = tablerIcon(IconKeyboard, IconKeyboardFilled);
const LockKey = tablerIcon(IconLock, IconLockFilled);
const Question = tablerIcon(IconHelpCircle, IconHelpCircleFilled);
const ShieldCheck = tablerIcon(IconShieldCheck);
const SignIn = tablerIcon(IconLogin);
const SignOut = tablerIcon(IconLogout);
const SpinnerGap = tablerIcon(IconLoader2);
const Trash = tablerIcon(IconTrash, IconTrashFilled);
const UserCircle = tablerIcon(IconUserCircle);
const WarningCircle = tablerIcon(IconAlertCircle, IconAlertCircleFilled);
const XCircle = tablerIcon(IconCircleX, IconCircleXFilled);

function SyncIndicator({ state, lastSynced }: { state: "unknown" | "synced" | "not-synced" | "failed"; lastSynced?: string }) {
  const syncedAt = lastSynced && lastSynced !== "Synced just now" ? new Date(lastSynced).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : lastSynced;
  if (state === "synced") return <span className="sync-status synced"><CheckCircle size={13} weight="fill" />{syncedAt === "Synced just now" ? syncedAt : `Synced ${syncedAt}`}</span>;
  if (state === "unknown") return <span className="sync-status unknown">Choose a project to check sync.</span>;
  return <span className={`sync-status ${state}`}><XCircle size={13} weight="fill" />{state === "failed" ? "Sync failed" : "Not synced yet"}</span>;
}

function App() {
  const [scan, setScan] = useState<ProjectScan>();
  const [recents, setRecents] = useState<RecentProject[]>([]);
  const [version, setVersion] = useState("");
  const [build, setBuild] = useState("");
  const [autoBuild, setAutoBuild] = useState(true);
  const [remember, setRemember] = useState(true);
  const [preview, setPreview] = useState<Preview>();
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);
  const [projectPath, setProjectPath] = useState("");
  const [page, setPage] = useState<Page>("overview");
  const [toast, setToast] = useState<Toast>();
  const [autoUpdate, setAutoUpdate] = useState(true);
  const [autoSyncAfterApply, setAutoSyncAfterApply] = useState(true);
  const [uploadBackupFiles, setUploadBackupFiles] = useState(false);
  const [saveProjects, setSaveProjects] = useState(true);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [availableUpdate, setAvailableUpdate] = useState<AvailableUpdate>();
  const [checkReport, setCheckReport] = useState<CheckReport>();
  const [checkBusy, setCheckBusy] = useState(false);
  const [projectTab, setProjectTab] = useState<"version" | "configuration" | "check" | "rename">("version");
  const [renameMode, setRenameMode] = useState<"name" | "id">("name");
  const [renameValue, setRenameValue] = useState("");
  const [renamePreview, setRenamePreview] = useState<RenamePreview>();
  const [renameBusy, setRenameBusy] = useState(false);
  const [account, setAccount] = useState<KlenuaAccount>();
  const [loginOpen, setLoginOpen] = useState(false);
  const [accountBusy, setAccountBusy] = useState(false);
  const [lastSynced, setLastSynced] = useState<string>();
  const [syncState, setSyncState] = useState<"unknown" | "synced" | "not-synced" | "failed">("unknown");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [staySignedIn, setStaySignedIn] = useState(true);
  const showToast = (title: string, message: string, tone: Toast["tone"] = "success") => {
    setToast({ title, message, tone });
    window.setTimeout(() => setToast(undefined), 4200);
  };
  const refreshRecents = async () => setRecents(await api<RecentProject[]>("get_recent_projects"));
  const refreshSyncStatus = async (root: string) => {
    try {
      const status = await api<KlenuaSyncStatus | null>("get_klenua_sync_status", { root });
      setLastSynced(status?.syncedAt);
      setSyncState(status ? "synced" : "not-synced");
    } catch {
      setLastSynced(undefined);
      setSyncState("failed");
    }
  };
  const checkForUpdates = async (quiet = false) => {
    setCheckingUpdates(true);
    try {
      const update = await check();
      if (!update) {
        setAvailableUpdate(undefined);
        if (!quiet) showToast("You’re up to date", "klenua is already on the latest release.", "info");
        return;
      }
      const next = { version: update.version, notes: update.body };
      setAvailableUpdate(next);
      if (!quiet) showToast("Update available", `klenua ${next.version} is ready to install.`, "info");
    } catch (error) { if (!quiet) showToast("Couldn’t check for updates", String(error), "error"); }
    finally { setCheckingUpdates(false); }
  };
  const installUpdate = async () => {
    setCheckingUpdates(true);
    try {
      const update = await check();
      if (!update) { setAvailableUpdate(undefined); showToast("You’re up to date", "No update is available.", "info"); return; }
      showToast("Installing update", `Downloading klenua ${update.version}…`, "info");
      await update.downloadAndInstall();
      await relaunch();
    } catch (error) { showToast("Update failed", String(error), "error"); }
    finally { setCheckingUpdates(false); }
  };
  const scanProject = async (path: string): Promise<ProjectScan | undefined> => {
    setBusy(true); setNotice(""); setPreview(undefined); setCheckReport(undefined); setRenamePreview(undefined); setRenameValue(""); setProjectTab("version");
    try {
      const result = await api<ProjectScan>("scan_project", { path });
      setScan(result); setVersion(result.version ?? ""); setBuild(result.build ?? "");
      if (account) void refreshSyncStatus(result.root);
      await refreshRecents(); setPage("project");
      showToast("Project scanned", `${result.projectName} is ready to update.`);
      if (result.warnings.length) setNotice(result.warnings.join(" "));
      return result;
    } catch (error) { setNotice(String(error)); showToast("Couldn’t scan project", String(error), "error"); return undefined; }
    finally { setBusy(false); }
  };
  const chooseProject = () => { setProjectPath(scan?.root ?? ""); setPickerOpen(true); };
  const browseForProject = async () => {
    try {
      const selected = await api<string | null>("open_project_folder");
      if (selected) { setProjectPath(selected); setPickerOpen(false); await scanProject(selected); }
    } catch (error) { setNotice(`Could not open the project picker: ${String(error)}`); showToast("Project picker unavailable", "Paste a local project path instead.", "error"); }
  };
  const openPath = async () => { if (projectPath.trim()) { setPickerOpen(false); await scanProject(projectPath.trim()); } };
  const makePreview = async () => {
    if (!scan) return;
    setBusy(true); setNotice("");
    try { setPreview(await api<Preview>("preview_changes", { path: scan.root, version, build })); }
    catch (error) { setNotice(String(error)); }
    finally { setBusy(false); }
  };
  const apply = async () => {
    if (!scan) return;
    if (!preview) { await makePreview(); return; }
    setBusy(true); setNotice("");
    try {
      const message = await api<string>("apply_changes", { path: scan.root, version, build, remember });
      setNotice(message); setPreview(undefined); showToast("Changes applied", "A backup was saved for every modified file."); const updated = await scanProject(scan.root); if (account && updated) { if (autoSyncAfterApply) await syncProject(updated, true); if (uploadBackupFiles) await uploadProjectBackups(updated.root); }
    } catch (error) { setNotice(String(error)); showToast("Changes not applied", String(error), "error"); }
    finally { setBusy(false); }
  };
  const restore = async () => {
    if (!scan) return;
    setBusy(true);
    try { setNotice(await api<string>("restore_last_change", { path: scan.root })); showToast("Last change restored", "Project files were restored from the klenua backup."); await scanProject(scan.root); }
    catch (error) { setNotice(String(error)); showToast("Restore unavailable", String(error), "error"); }
    finally { setBusy(false); }
  };
  const runProjectCheck = async () => {
    if (!scan) return;
    setCheckBusy(true);
    try { setCheckReport(await api<CheckReport>("run_check", { path: scan.root })); }
    catch (error) { showToast("Check failed", String(error), "error"); }
    finally { setCheckBusy(false); }
  };
  const previewRename = async () => {
    if (!scan || !renameValue.trim()) return;
    setRenameBusy(true); setNotice("");
    try {
      const command = renameMode === "name" ? "preview_rename_name" : "preview_rename_id";
      const args = renameMode === "name" ? { path: scan.root, name: renameValue } : { path: scan.root, id: renameValue };
      setRenamePreview(await api<RenamePreview>(command, args));
    } catch (error) { setNotice(String(error)); showToast("Couldn’t preview rename", String(error), "error"); }
    finally { setRenameBusy(false); }
  };
  const applyRename = async () => {
    if (!scan) return;
    setRenameBusy(true); setNotice("");
    try {
      const command = renameMode === "name" ? "apply_rename_name" : "apply_rename_id";
      const args = renameMode === "name" ? { path: scan.root, name: renameValue } : { path: scan.root, id: renameValue };
      const message = await api<string>(command, args);
      setNotice(message); setRenamePreview(undefined); setRenameValue("");
      showToast("Rename applied", "A backup was saved for every modified file.");
      const updated = await scanProject(scan.root);
      if (account && updated) { if (autoSyncAfterApply) await syncProject(updated, true); if (uploadBackupFiles) await uploadProjectBackups(updated.root); }
    } catch (error) { setNotice(String(error)); showToast("Rename not applied", String(error), "error"); }
    finally { setRenameBusy(false); }
  };
  const removeProject = async () => {
    if (!scan || !window.confirm(`Remove ${scan.projectName} from Saved projects? Its folder and files will not be deleted.`)) return;
    setBusy(true);
    try {
      await api<void>("remove_recent_project", { path: scan.root });
      await refreshRecents(); setScan(undefined); setPreview(undefined); setPage("overview");
      showToast("Project removed", `${scan.projectName} was removed from Saved projects.`, "info");
    } catch (error) { showToast("Couldn’t remove project", String(error), "error"); }
    finally { setBusy(false); }
  };
  const updateSettings = async (settings: Partial<AppSettings>) => {
    try { await api<void>("save_app_settings", { settings: { saveProjects, autoUpdate, autoSyncAfterApply, uploadBackupFiles, ...settings } }); }
    catch (error) { showToast("Couldn’t save settings", String(error), "error"); }
  };
  const signIn = async (event: React.FormEvent) => {
    event.preventDefault(); setAccountBusy(true);
    try {
      const signedIn = await api<KlenuaAccount>("login_klenua", { email, password, staySignedIn });
      setAccount(signedIn); setPassword(""); setLoginOpen(false); if (scan) void refreshSyncStatus(scan.root); showToast("Connected to klenua", `Signed in as ${signedIn.name}.`);
    } catch (error) { showToast("Couldn’t sign in", String(error), "error"); }
    finally { setAccountBusy(false); }
  };
  const signOut = async () => { await api<void>("logout_klenua"); setAccount(undefined); setLastSynced(undefined); setSyncState("unknown"); showToast("Signed out", "Your klenua session was disconnected.", "info"); };
  const syncProject = async (project?: ProjectScan | React.MouseEvent<HTMLButtonElement>, quiet = false) => {
    const target = project && "root" in project ? project : scan;
    if (!target) return;
    if (!account) { setLoginOpen(true); return; }
    setAccountBusy(true);
    try { const message = await api<string>("sync_klenua_project", { project: target }); setLastSynced("Synced just now"); setSyncState("synced"); if (!quiet) showToast("Project synced", message); }
    catch (error) { setSyncState("failed"); showToast("Couldn’t sync project", String(error), "error"); }
    finally { setAccountBusy(false); }
  };
  const uploadProjectBackups = async (root: string) => {
    setAccountBusy(true);
    try { const message = await api<string>("upload_klenua_backups", { root }); showToast("Backups uploaded", message); }
    catch (error) { showToast("Backup upload failed", String(error), "error"); }
    finally { setAccountBusy(false); }
  };
  useEffect(() => {
    void refreshRecents().catch(() => undefined);
    api<AppSettings>("get_app_settings").then(settings => { setSaveProjects(settings.saveProjects); setAutoUpdate(settings.autoUpdate); setAutoSyncAfterApply(settings.autoSyncAfterApply); setUploadBackupFiles(settings.uploadBackupFiles); if (settings.autoUpdate) void checkForUpdates(true); }).catch(() => undefined);
    api<KlenuaAccount | null>("get_klenua_account").then(value => setAccount(value ?? undefined)).catch(() => undefined);
  }, []);
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;
      if (event.code === "KeyO") { event.preventDefault(); event.stopPropagation(); chooseProject(); return; }
      if (event.code === "Enter" || event.code === "NumpadEnter") { event.preventDefault(); event.stopPropagation(); void apply(); return; }
      if (event.shiftKey && event.code === "KeyB") { event.preventDefault(); event.stopPropagation(); setBuild(value => String((Number(value) || 0) + 1)); }
    };
    window.addEventListener("keydown", handler, true);
    return () => window.removeEventListener("keydown", handler, true);
  }, [scan, preview, version, build]);
  const changedVersion = useMemo(() => scan && version !== scan.version, [scan, version]);
  const updateVersion = (next: string) => { setVersion(next); if (autoBuild && scan && next !== scan.version) setBuild(String((Number(scan.build) || 0) + 1)); };
  const latest = (scan ?? recents[0]) as (ProjectScan & Partial<RecentProject>) | (RecentProject & Partial<ProjectScan>) | undefined;
  const latestIsScanned = Boolean(scan && latest && "root" in latest);

  return <main>
    <header data-tauri-drag-region="deep"><div className="brand"><img className="brand-logo" src={kleunaLogo} alt="klenua" /></div><button className="open" onClick={chooseProject} disabled={busy}>{busy ? <SpinnerGap size={14} className="spin" /> : <FolderOpen size={14} weight="bold" />}{busy ? "Working…" : "Open Project"} <kbd>⌘ O</kbd></button></header>
    <section className="layout">
      <aside><div className="workspace-name sync-identity"><span className="workspace-icon"><UserCircle size={16} weight={account ? "fill" : "regular"} /></span><div><strong>{account ? account.name : "Not signed in"}</strong>{account ? <SyncIndicator state={syncState} lastSynced={lastSynced} /> : <small>Sync not connected</small>}</div></div><nav><button className={`nav-item ${page === "overview" ? "active" : ""}`} onClick={() => setPage("overview")}><Gauge size={17} weight={page === "overview" ? "fill" : "regular"} />Overview</button><button className={`nav-item ${page === "project" ? "active" : ""}`} onClick={() => scan ? setPage("project") : chooseProject()}><FolderOpen size={17} weight={page === "project" ? "fill" : "regular"} />Current project</button></nav><p className="eyebrow">Saved projects</p>{recents.length ? recents.map(project => { const active = page === "project" && scan?.root === project.path; return <button className={`recent ${active ? "active" : ""}`} key={project.path} onClick={() => void scanProject(project.path)}><ClockCounterClockwise size={14} weight={active ? "fill" : "regular"} /><span><strong>{project.name}</strong><small>{project.path}</small></span></button>; }) : <p className="muted">Projects you open will appear here.</p>}<div className="side-bottom"><button className={`nav-item ${page === "settings" ? "active" : ""}`} onClick={() => setPage("settings")}><Gear size={17} weight={page === "settings" ? "fill" : "regular"} />Settings</button><button className={`nav-item ${page === "help" ? "active" : ""}`} onClick={() => setPage("help")}><Question size={17} weight={page === "help" ? "fill" : "regular"} />Help</button></div></aside>
      <div className="content">
        {page === "overview" && <section className="overview-page"><p className="eyebrow">Workspace</p><h1>Overview</h1><p className="page-intro">Your latest project, saved workspaces, and klenua releases.</p><div className="overview-grid"><article className="overview-card latest-project"><div className="card-heading"><span className="card-icon"><FolderOpen size={18} weight="fill" /></span><div><p>Latest project</p><h2>{latest ? latest.projectName ?? latest.name : "No project selected"}</h2></div></div>{latest ? <><p className="card-copy">{latestIsScanned ? (latest as ProjectScan).root : (latest as RecentProject).path}</p>{latestIsScanned && <div className="project-metrics"><span><small>Version</small><b>{(latest as ProjectScan).version ?? "—"}</b></span><span><small>Build</small><b>{(latest as ProjectScan).build ?? "—"}</b></span></div>}<button className="inline-button" onClick={() => latestIsScanned ? setPage("project") : void scanProject((latest as RecentProject).path)}>{latestIsScanned ? "Open project" : "Scan project"}<ArrowUpRight size={14} /></button></> : <><p className="card-copy">Open a local project to see safe version sources and build details.</p><button className="inline-button" onClick={chooseProject}>Open project <ArrowUpRight size={14} /></button></>}</article><article className="overview-card update-card"><div className="card-heading"><span className="card-icon"><DownloadSimple size={18} weight="fill" /></span><div><p>klenua update</p><h2>{availableUpdate ? `Version ${availableUpdate.version}` : "You’re up to date"}</h2></div></div><p className="card-copy">{availableUpdate?.notes || (availableUpdate ? "A signed update is ready to download and install." : "Check for the latest signed klenua release.")}</p><button className="inline-button" disabled={checkingUpdates} onClick={availableUpdate ? () => void installUpdate() : () => void checkForUpdates()}>{checkingUpdates ? <SpinnerGap size={14} className="spin" /> : availableUpdate ? <DownloadSimple size={14} /> : <ArrowClockwise size={14} />}{checkingUpdates ? "Checking…" : availableUpdate ? "Install update" : "Check now"}</button></article></div><section className="saved-projects"><div className="section-title"><h2>Saved projects</h2><span>{recents.length} project{recents.length === 1 ? "" : "s"}</span></div>{recents.length ? recents.map(project => <button className="saved-project" key={project.path} onClick={() => void scanProject(project.path)}><span className="saved-project-icon"><FolderOpen size={16} /></span><span><strong>{project.name}</strong><small>{project.path}</small></span><ArrowUpRight size={15} /></button>) : <p className="empty-row">Saved projects will appear here after you open one.</p>}</section><section className="account-strip"><div><UserCircle size={20} weight="fill" /><span><strong>{account ? `Connected as ${account.name}` : "Connect your klenua account"}</strong><small>{account ? account.email : "Sign in to sync the current project without sharing its local path."}</small></span></div><button className="inline-button" onClick={account ? syncProject : () => setLoginOpen(true)} disabled={accountBusy || (Boolean(account) && !scan)}>{accountBusy ? <SpinnerGap size={14} className="spin" /> : <CloudArrowUp size={14} />}{account ? "Sync current project" : "Sign in to sync"}</button></section></section>}
        {page === "settings" && <section className="static-page"><p className="eyebrow">Preferences</p><h1>Settings</h1><p className="page-intro">Customize how klenua behaves on this computer.</p><div className="setting-group"><div className="setting-heading"><FolderOpen size={18} /><div><h2>Projects</h2><p>Choose whether opened projects are kept in your local workspace.</p></div></div><div className="setting-row"><div><strong>Save opened projects</strong><span>Keep scanned projects in Saved projects on this device.</span></div><label className="switch"><input type="checkbox" checked={saveProjects} onChange={event => { const next = event.target.checked; setSaveProjects(next); void updateSettings({ saveProjects: next, autoUpdate }); showToast(next ? "Projects will be saved" : "Project saving paused", next ? "New scans will appear in Saved projects." : "New projects will not be added to the list.", "info"); }} /><i /></label></div></div><div className="setting-group"><div className="setting-heading"><Bell size={18} /><div><h2>Updates</h2><p>Keep klenua current when a signed release is available.</p></div></div><div className="setting-row"><div><strong>Automatically check for updates</strong><span>Check after klenua launches.</span></div><label className="switch"><input type="checkbox" checked={autoUpdate} onChange={event => { const next = event.target.checked; setAutoUpdate(next); void updateSettings({ saveProjects, autoUpdate: next }); showToast(next ? "Automatic updates enabled" : "Automatic updates paused", next ? "klenua will check for signed releases at launch." : "You can check manually at any time.", "info"); }} /><i /></label></div><div className="setting-row"><div><strong>{availableUpdate ? `klenua ${availableUpdate.version} is ready` : "Check for updates"}</strong><span>{availableUpdate ? availableUpdate.notes || "Download and restart to complete the update." : "Check the configured signed release feed."}</span></div><button className="inline-button" disabled={checkingUpdates} onClick={availableUpdate ? () => void installUpdate() : () => void checkForUpdates()}><ArrowClockwise size={14} />{checkingUpdates ? "Checking…" : availableUpdate ? "Install update" : "Check now"}</button></div></div><div className="setting-group"><div className="setting-heading"><UserCircle size={18} /><div><h2>klenua account</h2><p>Optionally connect to sync project metadata to your account.</p></div></div><div className="setting-row"><div><strong>{account ? account.name : "Not connected"}</strong><span>{account ? `${account.email} · Your login is kept only for this app session.` : "Sign in with your regular klenua.com account."}</span></div><button className="inline-button" onClick={account ? signOut : () => setLoginOpen(true)}>{account ? <SignOut size={14} /> : <SignIn size={14} />}{account ? "Sign out" : "Sign in"}</button></div></div><div className="setting-group"><div className="setting-heading"><LockKey size={18} /><div><h2>Privacy</h2><p>klenua is a local developer tool.</p></div></div><div className="setting-row"><div><strong>Local-only project scanning</strong><span>Project files, paths, versions and backups never leave this device unless you explicitly sync a project.</span></div><CheckCircle className="setting-check" size={20} weight="fill" /></div></div></section>}
        {page === "help" && <section className="static-page"><p className="eyebrow">Support</p><h1>How can we help?</h1><p className="page-intro">Everything you need to safely manage project versions.</p><div className="help-grid"><article><FolderOpen size={19} /><h2>Open a project</h2><p>Choose a project folder or paste its full local path. klenua scans only known version sources.</p></article><article><ArrowClockwise size={19} /><h2>Preview before applying</h2><p>Every update is prepared in memory and displayed as a file-by-file diff before it can be written.</p></article><article><LockKey size={19} /><h2>Restore a change</h2><p>Each modified file is copied into <code>.klenuabuild-backup</code>. Use Restore Last Change to put it back.</p></article><article><CloudArrowUp size={19} /><h2>Account sync</h2><p>Sign in, then explicitly sync a project. Only project metadata is sent—never its local path or files.</p></article></div></section>}
        {page === "project" && !scan && <div className="empty"><span className="empty-icon"><FolderOpen size={19} weight="bold" /></span><h1>Your version workspace is ready.</h1><p>Open a local app project to find its version sources and prepare safe version changes.</p><button className="primary" onClick={chooseProject}>Open a project <ArrowUpRight size={14} /></button></div>}
        {page === "project" && scan && <><div className="project-head"><div><p className="eyebrow">Project</p><h1>{scan.projectName}</h1><p className="path">{scan.root}</p>{account && <SyncIndicator state={syncState} lastSynced={lastSynced} />}</div><div className="project-meta">{scan.git && <div className="git"><span>Git branch: <b>{scan.git.branch ?? "—"}</b></span><span>{scan.git.status}</span></div>}</div></div><div className="summary"><div><p>Detected</p><strong>{scan.projectType}</strong><div className="pills">{scan.platforms.map(platform => <span key={platform}>{platform}</span>)}</div></div><div><p>Current version</p><strong>{scan.version ?? "Not found"}</strong></div><div><p>Current build</p><strong>{scan.build ?? "Not found"}</strong></div><div><p>Source of truth</p><strong className="source">{scan.sourceOfTruth ?? "Manual review"}</strong></div></div><div className="project-tabs">
          <button className={`project-tab ${projectTab === "version" ? "active" : ""}`} onClick={() => setProjectTab("version")}>Version</button>
          <button className={`project-tab ${projectTab === "configuration" ? "active" : ""}`} onClick={() => setProjectTab("configuration")}>Configuration{scan.warnings.length > 0 && <span className="tab-dot tab-dot-warning" />}</button>
          <button className={`project-tab ${projectTab === "check" ? "active" : ""}`} onClick={() => setProjectTab("check")}>Check{checkReport && checkReport.findings.length > 0 && <span className={`tab-dot ${checkReport.hasErrors ? "tab-dot-error" : "tab-dot-warning"}`} />}</button>
          <button className={`project-tab ${projectTab === "rename" ? "active" : ""}`} onClick={() => setProjectTab("rename")}>Rename</button>
        </div>
        {notice && <p className="notice">{notice}</p>}
        {projectTab === "version" && <>
          <div className="editor"><label>Version<input value={version} placeholder="1.5.0" onChange={event => updateVersion(event.target.value)} /></label><label>Build<input value={build} placeholder="39" inputMode="numeric" onChange={event => setBuild(event.target.value)} /></label><div className="increments"><button onClick={() => updateVersion(bump(version, "patch"))} disabled={busy}>Patch +1</button><button onClick={() => updateVersion(bump(version, "minor"))} disabled={busy}>Minor +1</button><button onClick={() => updateVersion(bump(version, "major"))} disabled={busy}>Major +1</button><button onClick={() => setBuild(value => String((Number(value) || 0) + 1))} disabled={busy}>Build +1</button></div></div>
          <div className="options"><label><input type="checkbox" checked={autoBuild} onChange={event => setAutoBuild(event.target.checked)} /> Auto increment build when version changes</label><label><input type="checkbox" checked={remember} onChange={event => setRemember(event.target.checked)} /> Remember configuration for this project</label></div>
          <footer><button onClick={() => void makePreview()} disabled={busy}>{busy ? <><SpinnerGap size={14} className="spin" /> Preparing…</> : "Preview Changes"}</button><button className="primary" onClick={() => void apply()} disabled={busy || (!preview && !changedVersion && build === scan.build)}>{busy ? <><SpinnerGap size={14} className="spin" /> Working…</> : <>{preview ? "Apply Changes" : "Preview before applying"} <kbd>⌘ ↵</kbd></>}</button></footer>
          {preview && <section className="preview"><div className="section-title"><h2>Change preview</h2><span>{preview.changes.length} file{preview.changes.length === 1 ? "" : "s"}</span></div>{preview.warnings.map(warning => <p className="notice" key={warning}>{warning}</p>)}{preview.changes.map(file => <div className="diff" key={file.file}><p>{file.file}</p>{file.changes.map((change, index) => <React.Fragment key={index}><code className="before">− {change.before}</code><code className="after">+ {change.after}</code></React.Fragment>)}</div>)}</section>}
        </>}
        {projectTab === "configuration" && <section className="targets"><div className="section-title"><h2>Detected configuration</h2><div><button className="text-button" onClick={() => void syncProject()} disabled={accountBusy}>{accountBusy ? <SpinnerGap size={14} className="spin" /> : <CloudArrowUp size={14} />}{accountBusy ? "Syncing…" : "Sync"}</button><button className="text-button" onClick={() => void restore()} disabled={busy}>{busy ? <SpinnerGap size={14} className="spin" /> : "Restore Last Change"}</button></div></div>{scan.targets.map(target => <div className="target" key={target.id}><span className={target.editable ? "check" : "warn"}>{target.editable ? <CheckCircle size={16} weight="fill" /> : <WarningCircle size={16} weight="fill" />}</span><div><strong>{target.platform}</strong><span>{target.file}</span></div><em>{target.sourceOfTruth ? "Source of truth" : target.note ?? (target.editable ? "Will update" : "Manual review required")}</em></div>)}</section>}
        {projectTab === "check" && <section className="targets check-panel"><div className="section-title"><h2>Release check</h2><button className="text-button" onClick={() => void runProjectCheck()} disabled={checkBusy}>{checkBusy ? <SpinnerGap size={14} className="spin" /> : <ShieldCheck size={14} />}{checkBusy ? "Checking…" : "Run Check"}</button></div>{!checkReport && <p className="empty-row">Run a preflight check before release: version consistency, placeholder IDs, signing, and more.</p>}{checkReport && checkReport.findings.length === 0 && <p className="empty-row check-clean"><CheckCircle size={16} weight="fill" />No issues found.</p>}{checkReport?.findings.map((f, index) => <div className={`finding finding-${f.severity}`} key={index}><span className="finding-icon">{f.severity === "error" ? <XCircle size={16} weight="fill" /> : f.severity === "warning" ? <WarningCircle size={16} weight="fill" /> : <InfoCircle size={16} weight="fill" />}</span><div><strong>{f.message}</strong><span className="finding-meta">{f.rule} · {f.file}</span><span className="finding-hint">{f.fixHint}</span></div></div>)}</section>}
        {projectTab === "rename" && <section className="targets rename-panel">
          <div className="section-title"><h2>Rename</h2></div>
          <div className="rename-mode-toggle">
            <button className={renameMode === "name" ? "active" : ""} onClick={() => { setRenameMode("name"); setRenamePreview(undefined); setRenameValue(""); }}>Display name</button>
            <button className={renameMode === "id" ? "active" : ""} onClick={() => { setRenameMode("id"); setRenamePreview(undefined); setRenameValue(""); }}>Bundle / application ID</button>
          </div>
          <div className="rename-input-row">
            <input value={renameValue} placeholder={renameMode === "name" ? "New app name" : "com.company.app"} onChange={event => { setRenameValue(event.target.value); setRenamePreview(undefined); }} />
            <button className="primary" onClick={() => void previewRename()} disabled={renameBusy || !renameValue.trim()}>{renameBusy ? <SpinnerGap size={14} className="spin" /> : "Preview Rename"}</button>
          </div>
          {renamePreview && <>
            {renamePreview.warning && <p className="notice rename-warning"><WarningCircle size={14} />{renamePreview.warning}</p>}
            <section className="preview"><div className="section-title"><h2>Change preview</h2><span>{renamePreview.preview.changes.length} file{renamePreview.preview.changes.length === 1 ? "" : "s"}</span></div>{renamePreview.preview.warnings.map(warning => <p className="notice" key={warning}>{warning}</p>)}{renamePreview.preview.changes.map(file => <div className="diff" key={file.file}><p>{file.file}</p>{file.changes.map((change, index) => <React.Fragment key={index}><code className="before">− {change.before}</code><code className="after">+ {change.after}</code></React.Fragment>)}</div>)}</section>
            {renamePreview.manualAttention.length > 0 && <div className="manual-attention"><h3>Needs manual attention</h3>{renamePreview.manualAttention.map((item, index) => <div className="manual-attention-item" key={index}><strong>{item.file}</strong><span>{item.reason}</span></div>)}</div>}
            {renamePreview.outOfScope.length > 0 && <ul className="out-of-scope">{renamePreview.outOfScope.map((note, index) => <li key={index}>{note}</li>)}</ul>}
            <footer><button className="primary" onClick={() => void applyRename()} disabled={renameBusy}>{renameBusy ? <><SpinnerGap size={14} className="spin" /> Working…</> : "Apply Rename"}</button></footer>
          </>}
        </section>}
      </>}
        {page === "settings" && <section className="auto-sync-settings"><div className="setting-group"><div className="setting-heading"><CloudArrowUp size={18} /><div><h2>klenua sync</h2><p>Choose how project metadata is kept up to date.</p></div></div><div className="setting-row"><div><strong>Automatically sync after Apply Changes</strong><span>When you are signed in, sync the new version and build metadata after a successful apply.</span></div><label className="switch"><input type="checkbox" checked={autoSyncAfterApply} onChange={event => { const next = event.target.checked; setAutoSyncAfterApply(next); void updateSettings({ autoSyncAfterApply: next }); showToast(next ? "Automatic sync enabled" : "Automatic sync paused", next ? "Applied changes will update your synced project metadata." : "Use Sync manually when you want to update metadata.", "info"); }} /><i /></label></div></div></section>}
        {page === "settings" && <section className="backup-upload-settings"><div className="setting-group"><div className="setting-heading"><LockKey size={18} /><div><h2>Backup uploads</h2><p>Keep an off-device copy of the latest klenua backups.</p></div></div><div className="setting-row"><div><strong>Upload backup files to klenua</strong><span>After Apply Changes, upload only files in .klenuabuild-backup. Project files and local paths are never uploaded. Limit: 50 files, 10 MB total.</span></div><label className="switch"><input type="checkbox" checked={uploadBackupFiles} onChange={event => { const next = event.target.checked; setUploadBackupFiles(next); void updateSettings({ uploadBackupFiles: next }); showToast(next ? "Backup uploads enabled" : "Backup uploads paused", next ? "Latest klenua backups will upload after Apply Changes." : "Backup files will remain only on this device.", "info"); }} /><i /></label></div></div></section>}
        {page === "project" && scan && <button className="remove-project" onClick={() => void removeProject()} disabled={busy}><Trash size={14} />Remove from Saved projects</button>}
      </div>
    </section>
    {toast && <div className={`toast ${toast.tone}`} role="status"><CheckCircle size={18} weight="fill" /><div><strong>{toast.title}</strong><span>{toast.message}</span></div><button onClick={() => setToast(undefined)} aria-label="Dismiss notification">×</button></div>}
    {pickerOpen && <div className="modal-backdrop" role="presentation"><section className="path-modal" role="dialog" aria-modal="true" aria-label="Open project"><button className="modal-close" onClick={() => setPickerOpen(false)} aria-label="Close" disabled={busy}>×</button><div className="modal-mark"><FolderOpen size={17} weight="bold" /></div><h2>Open a project folder</h2><p>Choose a folder in Finder, or paste a local path directly.</p><label>Project folder<input autoFocus value={projectPath} placeholder="/Users/you/Projects/MyApp" onChange={event => setProjectPath(event.target.value)} onKeyDown={event => { if (event.key === "Enter") void openPath(); }} disabled={busy} /></label><div className="modal-actions"><button onClick={() => setPickerOpen(false)} disabled={busy}>Cancel</button><button onClick={() => void browseForProject()} disabled={busy}>{busy ? <SpinnerGap size={14} className="spin" /> : null}Browse…</button><button className="primary" onClick={() => void openPath()} disabled={!projectPath.trim() || busy}>{busy ? <SpinnerGap size={14} className="spin" /> : null}{busy ? "Opening…" : "Open Project"}</button></div></section></div>}
    {loginOpen && <div className="modal-backdrop" role="presentation"><form className="path-modal login-modal" role="dialog" aria-modal="true" aria-label="Sign in to klenua" onSubmit={signIn}><button type="button" className="modal-close" onClick={() => setLoginOpen(false)} aria-label="Close" disabled={accountBusy}>×</button><div className="modal-mark"><UserCircle size={18} weight="fill" /></div><h2>Sign in to klenua</h2><p>Use your regular klenua.com account. Your password is never stored.</p><label>Email<input autoFocus type="email" autoComplete="email" value={email} onChange={event => setEmail(event.target.value)} required disabled={accountBusy} /></label><label>Password<input type="password" autoComplete="current-password" value={password} onChange={event => setPassword(event.target.value)} required disabled={accountBusy} /></label><label className="remember-login"><input type="checkbox" checked={staySignedIn} onChange={event => setStaySignedIn(event.target.checked)} disabled={accountBusy} /><span><strong>Stay signed in for 30 days</strong><small>Stored securely in your system keychain.</small></span></label><div className="modal-actions"><button type="button" onClick={() => setLoginOpen(false)} disabled={accountBusy}>Cancel</button><button className="primary" disabled={accountBusy}>{accountBusy ? <><SpinnerGap size={14} className="spin" /> Signing in…</> : "Sign in"}</button></div></form></div>}
  </main>;
}
createRoot(document.getElementById("root")!).render(<App />);
