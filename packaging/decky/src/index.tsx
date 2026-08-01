import {
  ButtonItem,
  PanelSection,
  PanelSectionRow,
  TextField,
  ToggleField,
  staticClasses,
} from "@decky/ui";
import { callable, definePlugin, toaster } from "@decky/api";
import { useEffect, useState } from "react";
import { FaLightbulb } from "react-icons/fa";

type CaptureConfig = {
  hyperion_url: string;
  flatbuffer_port: number;
  priority: number;
  fps: number;
  output_height: number;
  drm_device: string;
  drm_connector: string;
  auto_start: boolean;
};

type Status = { installed: boolean; running: boolean; error: string };
type SaveResult = { ok: boolean; config: CaptureConfig | null; error: string };

const getStatus = callable<[], Status>("get_status");
const getConfig = callable<[], CaptureConfig>("get_config");
const saveConfig = callable<[value: CaptureConfig], SaveResult>("save_config");
const startCapture = callable<[], boolean>("start_capture");
const stopCapture = callable<[], boolean>("stop_capture");

const defaults: CaptureConfig = {
  hyperion_url: "http://127.0.0.1:8090/",
  flatbuffer_port: 19400,
  priority: 150,
  fps: 20,
  output_height: 480,
  drm_device: "/dev/dri/card0",
  drm_connector: "",
  auto_start: true,
};

function Content() {
  const [config, setConfig] = useState<CaptureConfig>(defaults);
  const [status, setStatus] = useState<Status>({ installed: false, running: false, error: "" });
  const [busy, setBusy] = useState(false);

  const refresh = async () => setStatus(await getStatus());

  useEffect(() => {
    void Promise.all([getConfig(), getStatus()]).then(([loadedConfig, loadedStatus]) => {
      setConfig(loadedConfig);
      setStatus(loadedStatus);
    });
  }, []);

  const save = async () => {
    setBusy(true);
    const result = await saveConfig(config);
    setBusy(false);
    if (result.ok && result.config) {
      setConfig(result.config);
      toaster.toast({ title: "Hyperion Capture", body: "Configuration saved" });
    } else {
      toaster.toast({ title: "Invalid configuration", body: result.error });
    }
  };

  const toggleCapture = async () => {
    setBusy(true);
    const ok = status.running ? await stopCapture() : await startCapture();
    await refresh();
    setBusy(false);
    if (!ok) {
      toaster.toast({ title: "Hyperion Capture", body: "Operation failed; check the status and logs" });
    }
  };

  return (
    <>
      <PanelSection title="Status">
        <PanelSectionRow>
          <ButtonItem layout="below" disabled={busy} onClick={toggleCapture}>
            {status.running ? "Stop capture" : "Start capture"}
          </ButtonItem>
        </PanelSectionRow>
        {status.error && <PanelSectionRow>{status.error}</PanelSectionRow>}
      </PanelSection>
      <PanelSection title="Hyperion">
        <PanelSectionRow>
          <TextField
            label="Server URL"
            value={config.hyperion_url}
            onChange={(event) => setConfig({ ...config, hyperion_url: event.target.value })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="FlatBuffers port"
            value={String(config.flatbuffer_port)}
            onChange={(event) => setConfig({ ...config, flatbuffer_port: Number(event.target.value) })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Priority"
            value={String(config.priority)}
            onChange={(event) => setConfig({ ...config, priority: Number(event.target.value) })}
          />
        </PanelSectionRow>
      </PanelSection>
      <PanelSection title="Capture">
        <PanelSectionRow>
          <TextField
            label="DRM device"
            value={config.drm_device}
            onChange={(event) => setConfig({ ...config, drm_device: event.target.value })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Connector (optional)"
            value={config.drm_connector}
            onChange={(event) => setConfig({ ...config, drm_connector: event.target.value })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Frames per second"
            value={String(config.fps)}
            onChange={(event) => setConfig({ ...config, fps: Number(event.target.value) })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <TextField
            label="Maximum output height"
            value={String(config.output_height)}
            onChange={(event) => setConfig({ ...config, output_height: Number(event.target.value) })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ToggleField
            label="Start with Decky"
            checked={config.auto_start}
            onChange={(auto_start) => setConfig({ ...config, auto_start })}
          />
        </PanelSectionRow>
        <PanelSectionRow>
          <ButtonItem layout="below" disabled={busy} onClick={save}>
            Save configuration
          </ButtonItem>
        </PanelSectionRow>
      </PanelSection>
    </>
  );
}

export default definePlugin(() => ({
  name: "Hyperion Capture",
  titleView: <div className={staticClasses.Title}>Hyperion Capture</div>,
  content: <Content />,
  icon: <FaLightbulb />,
}));
