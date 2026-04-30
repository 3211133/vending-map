const API_BASE = "/api";
const DEFAULT_CENTER = [35.681236, 139.767125];
const VOTED_KEY = "vending-map-voted";
const MAX_NAME_LENGTH = 80;
const MAX_OPERATOR_LENGTH = 80;
const MAX_OPENING_HOURS_LENGTH = 120;

const state = {
  map: null,
  clusterGroup: null,
  markersById: new Map(),
  featuresById: new Map(),
  addMode: false,
  draftMarker: null,
  selectedLatLng: null,
  fetchTimer: null,
  filterType: "",
  votedMap: loadVotedMap(),
  addFormStage: 1,
  userLocationMarker: null,
};

const elements = {
  filter: document.getElementById("vendingTypeFilter"),
  toggleAddModeButton: document.getElementById("toggleAddModeButton"),
  locateButton: document.getElementById("locateButton"),
  addMachineForm: document.getElementById("addMachineForm"),
  submitMachineButton: document.getElementById("submitMachineButton"),
  draftCoords: document.getElementById("draftCoords"),
  addMachineMessage: document.getElementById("addMachineMessage"),
  addMachineStep: document.getElementById("addMachineStep"),
  machineName: document.getElementById("machineName"),
  machineType: document.getElementById("machineType"),
  machineOperator: document.getElementById("machineOperator"),
  machineHours: document.getElementById("machineHours"),
  coordsError: document.getElementById("coordsError"),
  machineNameError: document.getElementById("machineNameError"),
  machineTypeError: document.getElementById("machineTypeError"),
  machineOperatorError: document.getElementById("machineOperatorError"),
  machineHoursError: document.getElementById("machineHoursError"),
  goStageLocation: document.getElementById("goStageLocation"),
  goStageDetails: document.getElementById("goStageDetails"),
  nextStageButton: document.getElementById("nextStageButton"),
  statusMessage: document.getElementById("statusMessage"),
  sidebar: document.getElementById("sidebar"),
  openSidebarButton: document.getElementById("openSidebarButton"),
  closeSidebarButton: document.getElementById("closeSidebarButton"),
};

initialize();

function initialize() {
  state.map = L.map("map", {
    zoomControl: true,
    minZoom: 4,
  }).setView(DEFAULT_CENTER, 15);

  L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
    maxZoom: 19,
    attribution: "&copy; OpenStreetMap contributors",
  }).addTo(state.map);

  state.clusterGroup = L.markerClusterGroup({
    disableClusteringAtZoom: 16,
    spiderfyOnMaxZoom: true,
  });
  state.map.addLayer(state.clusterGroup);

  bindEvents();
  fetchMachines();
}

function bindEvents() {
  state.map.on("moveend", () => {
    window.clearTimeout(state.fetchTimer);
    state.fetchTimer = window.setTimeout(fetchMachines, 500);
  });

  state.map.on("click", (event) => {
    if (!state.addMode) {
      return;
    }
    setDraftMarker(event.latlng);
  });

  elements.filter.addEventListener("change", () => {
    state.filterType = elements.filter.value;
    fetchMachines();
  });

  elements.toggleAddModeButton.addEventListener("click", () => {
    state.addMode = !state.addMode;
    elements.toggleAddModeButton.setAttribute("aria-pressed", String(state.addMode));
    elements.toggleAddModeButton.textContent = state.addMode
      ? "追加モード: ON"
      : "追加モード: OFF";
    updateStatus(state.addMode ? "地図をクリックして仮マーカーを置いてください" : "追加モードを終了しました");
    if (!state.addMode) {
      updateAddMachineStep("STEP 1: 地図で位置を選択");
    }
  });

  elements.locateButton.addEventListener("click", locateUser);
  elements.addMachineForm.addEventListener("submit", submitNewMachine);
  elements.machineName?.addEventListener("input", () => clearSingleFieldError("name"));
  elements.machineType?.addEventListener("change", () => clearSingleFieldError("vending_type"));
  elements.machineOperator?.addEventListener("input", () => clearSingleFieldError("operator"));
  elements.machineHours?.addEventListener("input", () => clearSingleFieldError("opening_hours"));
  elements.goStageLocation?.addEventListener("click", () => setAddFormStage(1));
  elements.goStageDetails?.addEventListener("click", () => setAddFormStage(2));
  elements.nextStageButton?.addEventListener("click", () => {
    clearFieldErrors();
    if (!state.selectedLatLng) {
      setFieldError("coords", "先に地図上で位置を選択してください。");
      updateAddMachineMessage("位置を選択後に次へ進んでください。");
      return;
    }
    setAddFormStage(2);
    updateAddMachineStep("STEP 2: 基本情報を入力");
    updateAddMachineMessage("必要項目を確認して追加してください。");
  });

  // Mobile sidebar toggle
  const backdrop = document.createElement("div");
  backdrop.className = "sidebar-backdrop";
  document.body.appendChild(backdrop);

  function openSidebar() {
    elements.sidebar.classList.add("is-open");
    elements.sidebar.setAttribute("aria-hidden", "false");
    elements.openSidebarButton?.setAttribute("aria-expanded", "true");
    backdrop.classList.add("is-visible");
    elements.closeSidebarButton?.focus();
  }

  function closeSidebar() {
    elements.sidebar.classList.remove("is-open");
    elements.sidebar.setAttribute("aria-hidden", "true");
    elements.openSidebarButton?.setAttribute("aria-expanded", "false");
    backdrop.classList.remove("is-visible");
    elements.openSidebarButton?.focus();
  }

  if (elements.openSidebarButton) {
    elements.openSidebarButton.addEventListener("click", openSidebar);
  }
  if (elements.closeSidebarButton) {
    elements.closeSidebarButton.addEventListener("click", closeSidebar);
  }
  backdrop.addEventListener("click", closeSidebar);
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      closeSidebar();
    }
  });

  syncSidebarA11yState();
  window.addEventListener("resize", syncSidebarA11yState);

  function syncSidebarA11yState() {
    const mobile = window.matchMedia("(max-width: 960px)").matches;
    if (!mobile) {
      elements.sidebar.setAttribute("aria-hidden", "false");
      elements.openSidebarButton?.setAttribute("aria-expanded", "false");
      return;
    }
    const isOpen = elements.sidebar.classList.contains("is-open");
    elements.sidebar.setAttribute("aria-hidden", String(!isOpen));
    elements.openSidebarButton?.setAttribute("aria-expanded", String(isOpen));
  }
}

async function fetchMachines() {
  const bounds = state.map.getBounds();
  const params = new URLSearchParams({
    south: String(bounds.getSouth()),
    west: String(bounds.getWest()),
    north: String(bounds.getNorth()),
    east: String(bounds.getEast()),
    limit: "500",
  });

  if (state.filterType) {
    params.set("vending_type", state.filterType);
  }

  updateStatus("自販機データを読み込み中...");

  try {
    const response = await fetch(`${API_BASE}/vending-machines?${params.toString()}`);
    if (!response.ok) {
      throw new Error("自販機一覧の取得に失敗しました");
    }

    const data = await response.json();
    renderFeatures(data.features || []);
    updateStatus(`${(data.features || []).length} 件を表示中`);
  } catch (error) {
    updateStatus(error.message);
  }
}

function renderFeatures(features) {
  state.clusterGroup.clearLayers();
  state.markersById.clear();
  state.featuresById.clear();

  features.forEach((feature) => {
    const marker = L.marker(
      [feature.geometry.coordinates[1], feature.geometry.coordinates[0]],
      { icon: createMarkerIcon(feature.properties) }
    );

    marker.bindPopup(buildPopupContent(feature));
    marker.on("popupopen", () => {
      const content = buildPopupContent(feature);
      marker.setPopupContent(content);
      loadImagesForPopup(feature.properties.id, content);
    });

    state.clusterGroup.addLayer(marker);
    state.markersById.set(feature.properties.id, marker);
    state.featuresById.set(feature.properties.id, feature);
  });
}

function createMarkerIcon(properties) {
  const marker = document.createElement("div");
  marker.className = "machine-marker";
  marker.style.backgroundColor = markerColor(properties);
  marker.dataset.source = properties.source;
  marker.textContent = markerSymbol(properties.status);

  return L.divIcon({
    html: marker.outerHTML,
    className: "",
    iconSize: [26, 26],
    iconAnchor: [13, 13],
    popupAnchor: [0, -10],
  });
}

function markerSymbol(status) {
  if (status === "removed") {
    return "×";
  }
  if (status === "suspected_removed") {
    return "!";
  }
  return "✓";
}

function markerColor(properties) {
  if (properties.status === "removed") {
    return "#7b8794";
  }
  if (properties.trust_score >= 70) {
    return "#1f8f5f";
  }
  if (properties.trust_score >= 40) {
    return "#d0a022";
  }
  return "#c5523b";
}

function buildPopupContent(feature) {
  const wrapper = document.createElement("div");
  wrapper.className = "popup-card";
  const idBase = String(feature.properties.id);

  const title = document.createElement("h3");
  title.textContent = feature.properties.name || "名称未設定";
  wrapper.appendChild(title);

  const tabNav = document.createElement("div");
  tabNav.className = "popup-tabs";
  tabNav.setAttribute("role", "tablist");

  const infoTab = document.createElement("button");
  infoTab.type = "button";
  infoTab.className = "popup-tab is-active";
  infoTab.setAttribute("role", "tab");
  infoTab.setAttribute("aria-selected", "true");
  infoTab.tabIndex = 0;
  infoTab.id = `tab-info-${idBase}`;
  infoTab.setAttribute("aria-controls", `panel-info-${idBase}`);
  infoTab.textContent = "情報";

  const mediaTab = document.createElement("button");
  mediaTab.type = "button";
  mediaTab.className = "popup-tab";
  mediaTab.setAttribute("role", "tab");
  mediaTab.setAttribute("aria-selected", "false");
  mediaTab.tabIndex = -1;
  mediaTab.id = `tab-media-${idBase}`;
  mediaTab.setAttribute("aria-controls", `panel-media-${idBase}`);
  mediaTab.textContent = "写真/OCR";

  tabNav.appendChild(infoTab);
  tabNav.appendChild(mediaTab);
  wrapper.appendChild(tabNav);

  const infoPanel = document.createElement("section");
  infoPanel.className = "popup-panel is-active";
  infoPanel.setAttribute("role", "tabpanel");
  infoPanel.id = `panel-info-${idBase}`;
  infoPanel.setAttribute("aria-labelledby", infoTab.id);

  const mediaPanel = document.createElement("section");
  mediaPanel.className = "popup-panel";
  mediaPanel.setAttribute("role", "tabpanel");
  mediaPanel.id = `panel-media-${idBase}`;
  mediaPanel.setAttribute("aria-labelledby", mediaTab.id);

  const type = document.createElement("p");
  type.textContent = `種別: ${feature.properties.vending_type}`;

  const score = document.createElement("p");
  score.textContent = `信頼度: ${feature.properties.trust_score} / 状態: ${statusLabel(feature.properties.status)}`;

  const votes = document.createElement("p");
  votes.textContent = `exists ${feature.properties.exists_count} / removed ${feature.properties.removed_count}`;

  const source = document.createElement("p");
  source.textContent = `source: ${feature.properties.source}`;

  infoPanel.appendChild(type);
  infoPanel.appendChild(score);
  infoPanel.appendChild(votes);
  infoPanel.appendChild(source);

  const votedToday = state.votedMap[feature.properties.id] === todayKey();
  const actions = document.createElement("div");
  actions.className = "popup-actions";

  const existsButton = document.createElement("button");
  existsButton.type = "button";
  existsButton.textContent = "まだある";
  existsButton.disabled = votedToday;
  existsButton.addEventListener("click", () => submitVote(feature.properties.id, "exists"));

  const removedButton = document.createElement("button");
  removedButton.type = "button";
  removedButton.textContent = "なくなってた";
  removedButton.disabled = votedToday;
  removedButton.addEventListener("click", () => submitVote(feature.properties.id, "removed"));

  actions.appendChild(existsButton);
  actions.appendChild(removedButton);
  infoPanel.appendChild(actions);

  // Image upload section
  const imageSection = document.createElement("div");
  imageSection.className = "popup-image-section";

  const imageListEl = document.createElement("div");
  imageListEl.className = "popup-images";
  imageListEl.textContent = "読み込み中...";
  imageListEl.setAttribute("aria-live", "polite");

  const fileInputId = `file-${feature.properties.id}`;
  const fileInput = document.createElement("input");
  fileInput.type = "file";
  fileInput.accept = "image/*";
  fileInput.id = fileInputId;
  fileInput.className = "popup-file-input";

  const fileLabel = document.createElement("label");
  fileLabel.htmlFor = fileInputId;
  fileLabel.textContent = "写真を選択";
  fileLabel.className = "popup-file-label";

  const uploadBtn = document.createElement("button");
  uploadBtn.type = "button";
  uploadBtn.textContent = "OCR解析";
  uploadBtn.className = "popup-upload-btn";
  uploadBtn.disabled = true;

  const itemsList = document.createElement("ul");
  itemsList.className = "popup-items-list";

  const uploadMessage = document.createElement("p");
  uploadMessage.className = "popup-helper";
  uploadMessage.textContent = "画像を選択して OCR 解析できます";

  fileInput.addEventListener("change", () => {
    uploadBtn.disabled = !fileInput.files?.length;
    uploadBtn.textContent = "OCR解析";
  });

  uploadBtn.addEventListener("click", () => {
    if (fileInput.files?.length) {
      uploadImage(feature.properties.id, fileInput.files[0], imageSection, uploadMessage);
    }
  });

  imageSection.appendChild(imageListEl);
  imageSection.appendChild(fileLabel);
  imageSection.appendChild(fileInput);
  imageSection.appendChild(uploadBtn);
  imageSection.appendChild(uploadMessage);
  imageSection.appendChild(itemsList);
  mediaPanel.appendChild(imageSection);
  wrapper.appendChild(infoPanel);
  wrapper.appendChild(mediaPanel);

  function activateTab(tabName) {
    const infoActive = tabName === "info";
    infoTab.classList.toggle("is-active", infoActive);
    infoTab.setAttribute("aria-selected", String(infoActive));
    infoTab.tabIndex = infoActive ? 0 : -1;
    mediaTab.classList.toggle("is-active", !infoActive);
    mediaTab.setAttribute("aria-selected", String(!infoActive));
    mediaTab.tabIndex = infoActive ? -1 : 0;
    infoPanel.classList.toggle("is-active", infoActive);
    mediaPanel.classList.toggle("is-active", !infoActive);
  }

  infoTab.addEventListener("click", () => activateTab("info"));
  mediaTab.addEventListener("click", () => activateTab("media"));
  tabNav.addEventListener("keydown", (event) => {
    if (event.key === "ArrowRight") {
      activateTab("media");
      mediaTab.focus();
    }
    if (event.key === "ArrowLeft") {
      activateTab("info");
      infoTab.focus();
    }
  });

  return wrapper;
}

function setDraftMarker(latlng) {
  state.selectedLatLng = latlng;
  elements.submitMachineButton.disabled = false;
  elements.draftCoords.textContent = `選択位置: ${latlng.lat.toFixed(6)}, ${latlng.lng.toFixed(6)}`;
  updateAddMachineStep("STEP 2: 基本情報を入力");
  updateAddMachineMessage("位置を選択しました。入力後に追加してください。");
  clearFieldErrors();
  setAddFormStage(2);

  if (state.draftMarker) {
    state.map.removeLayer(state.draftMarker);
  }

  state.draftMarker = L.marker(latlng, { draggable: true }).addTo(state.map);
  state.draftMarker.on("dragend", () => {
    const nextLatLng = state.draftMarker.getLatLng();
    state.selectedLatLng = nextLatLng;
    elements.draftCoords.textContent = `選択位置: ${nextLatLng.lat.toFixed(6)}, ${nextLatLng.lng.toFixed(6)}`;
  });
}

async function submitNewMachine(event) {
  event.preventDefault();
  clearFieldErrors();
  if (!state.selectedLatLng) {
    updateStatus("追加位置を先に選択してください");
    setFieldError("coords", "先に地図上で位置を選択してください。");
    updateAddMachineMessage("先に地図上で位置を選択してください。");
    setAddFormStage(1);
    return;
  }

  const formData = new FormData(elements.addMachineForm);
  const payload = {
    latitude: state.selectedLatLng.lat,
    longitude: state.selectedLatLng.lng,
    name: normalizeString(formData.get("name")),
    vending_type: normalizeString(formData.get("vending_type")) || "drinks",
    operator: normalizeString(formData.get("operator")),
    opening_hours: normalizeString(formData.get("opening_hours")),
  };
  const validationError = validateAddMachinePayload(payload);
  if (validationError) {
    setFieldError(validationError.field, validationError.message);
    updateAddMachineMessage(validationError.message);
    setAddFormStage(validationError.field === "coords" ? 1 : 2);
    return;
  }

  updateStatus("自販機を追加しています...");
  updateAddMachineMessage("送信中...");
  updateAddMachineStep("STEP 3: 確認中");

  try {
    const response = await fetch(`${API_BASE}/vending-machines`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload),
    });
    if (!response.ok) {
      const errorBody = await safeJson(response);
      throw new Error(readErrorMessage(errorBody, "自販機の追加に失敗しました"));
    }

    clearDraftState();
    await fetchMachines();
    updateStatus("自販機を追加しました");
    updateAddMachineMessage("追加が完了しました。");
    updateAddMachineStep("STEP 1: 地図で位置を選択");
  } catch (error) {
    applyAddMachineFieldErrors(error.message);
    updateStatus(error.message);
    updateAddMachineMessage(error.message);
    updateAddMachineStep("STEP 2: 基本情報を確認");
    setAddFormStage(2);
  }
}

async function submitVote(machineId, voteType) {
  updateStatus("投票を送信しています...");

  try {
    const response = await fetch(`${API_BASE}/vending-machines/${machineId}/votes`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ vote_type: voteType }),
    });

    if (!response.ok) {
      const errorBody = await safeJson(response);
      throw new Error(readErrorMessage(errorBody, "投票に失敗しました"));
    }

    state.votedMap[machineId] = todayKey();
    saveVotedMap(state.votedMap);
    await fetchMachines();
    updateStatus("投票を反映しました");
  } catch (error) {
    updateStatus(error.message);
  }
}

function locateUser() {
  if (!navigator.geolocation) {
    updateStatus("Geolocation API を利用できません");
    return;
  }

  if (!window.isSecureContext) {
    updateStatus("現在地取得には HTTPS か localhost が必要です（IP直アクセスでは動作しない場合があります）");
    return;
  }

  const locateButton = elements.locateButton;
  if (locateButton) {
    locateButton.disabled = true;
    locateButton.textContent = "現在地取得中...";
  }

  navigator.geolocation.getCurrentPosition(
    (position) => {
      const { latitude, longitude } = position.coords;
      state.map.flyTo([latitude, longitude], 17);
      renderUserLocationMarker(latitude, longitude, position.coords.accuracy);
      updateStatus("現在地へ移動しました");
      resetLocateButton();
    },
    (error) => {
      updateStatus(readGeolocationError(error));
      resetLocateButton();
    },
    { enableHighAccuracy: true, timeout: 10000 }
  );
}

function renderUserLocationMarker(latitude, longitude, accuracy) {
  if (state.userLocationMarker) {
    state.map.removeLayer(state.userLocationMarker);
  }

  const marker = L.circleMarker([latitude, longitude], {
    radius: 8,
    color: "#0c7f60",
    weight: 2,
    fillColor: "#13a07a",
    fillOpacity: 0.6,
  });
  marker.bindPopup(`現在地（誤差 ±${Math.round(accuracy)}m）`);
  marker.addTo(state.map);
  state.userLocationMarker = marker;
}

function readGeolocationError(error) {
  if (!error) {
    return "現在地を取得できませんでした";
  }
  if (error.code === error.PERMISSION_DENIED) {
    return "現在地アクセスが拒否されました。ブラウザ設定で位置情報許可を確認してください。";
  }
  if (error.code === error.POSITION_UNAVAILABLE) {
    return "現在地を特定できませんでした。電波状況や端末設定を確認してください。";
  }
  if (error.code === error.TIMEOUT) {
    return "現在地取得がタイムアウトしました。もう一度お試しください。";
  }
  return "現在地を取得できませんでした";
}

function resetLocateButton() {
  if (elements.locateButton) {
    elements.locateButton.disabled = false;
    elements.locateButton.textContent = "現在地へ";
  }
}

function clearDraftState() {
  elements.addMachineForm.reset();
  elements.draftCoords.textContent = "地図をクリックして位置を選択";
  elements.submitMachineButton.disabled = true;
  state.selectedLatLng = null;
  state.addFormStage = 1;
  elements.addMachineForm.setAttribute("data-stage", "1");
  updateAddMachineStep("STEP 1: 地図で位置を選択");
  clearFieldErrors();

  if (state.draftMarker) {
    state.map.removeLayer(state.draftMarker);
    state.draftMarker = null;
  }
}

function updateStatus(message) {
  elements.statusMessage.textContent = message;
}

function updateAddMachineMessage(message) {
  if (elements.addMachineMessage) {
    elements.addMachineMessage.textContent = message;
  }
}

function updateAddMachineStep(message) {
  if (elements.addMachineStep) {
    elements.addMachineStep.textContent = message;
  }
}

function setAddFormStage(stage) {
  state.addFormStage = stage;
  elements.addMachineForm?.setAttribute("data-stage", String(stage));
  if (elements.goStageLocation) {
    const active = stage === 1;
    elements.goStageLocation.classList.toggle("is-active", active);
    elements.goStageLocation.setAttribute("aria-selected", String(active));
  }
  if (elements.goStageDetails) {
    const active = stage === 2;
    elements.goStageDetails.classList.toggle("is-active", active);
    elements.goStageDetails.setAttribute("aria-selected", String(active));
  }
}

function clearFieldErrors() {
  [elements.coordsError, elements.machineNameError, elements.machineTypeError, elements.machineOperatorError, elements.machineHoursError]
    .forEach((el) => {
      if (el) {
        el.textContent = "";
      }
    });
}

function setFieldError(field, message) {
  if (field === "coords" && elements.coordsError) {
    elements.coordsError.textContent = message;
  }
  if (field === "name" && elements.machineNameError) {
    elements.machineNameError.textContent = message;
  }
  if (field === "vending_type" && elements.machineTypeError) {
    elements.machineTypeError.textContent = message;
  }
  if (field === "operator" && elements.machineOperatorError) {
    elements.machineOperatorError.textContent = message;
  }
  if (field === "opening_hours" && elements.machineHoursError) {
    elements.machineHoursError.textContent = message;
  }
}

function clearSingleFieldError(field) {
  if (field === "name" && elements.machineNameError) {
    elements.machineNameError.textContent = "";
  }
  if (field === "vending_type" && elements.machineTypeError) {
    elements.machineTypeError.textContent = "";
  }
  if (field === "operator" && elements.machineOperatorError) {
    elements.machineOperatorError.textContent = "";
  }
  if (field === "opening_hours" && elements.machineHoursError) {
    elements.machineHoursError.textContent = "";
  }
}

function validateAddMachinePayload(payload) {
  if (!payload.vending_type) {
    return { field: "vending_type", message: "商品種別を選択してください。" };
  }
  if (payload.name && payload.name.length > MAX_NAME_LENGTH) {
    return { field: "name", message: `名前は ${MAX_NAME_LENGTH} 文字以内で入力してください。` };
  }
  if (payload.operator && payload.operator.length > MAX_OPERATOR_LENGTH) {
    return { field: "operator", message: `運営者は ${MAX_OPERATOR_LENGTH} 文字以内で入力してください。` };
  }
  if (payload.opening_hours && payload.opening_hours.length > MAX_OPENING_HOURS_LENGTH) {
    return { field: "opening_hours", message: `営業時間は ${MAX_OPENING_HOURS_LENGTH} 文字以内で入力してください。` };
  }
  return null;
}

function applyAddMachineFieldErrors(message) {
  const lower = message.toLowerCase();
  if (lower.includes("coordinate") || lower.includes("bbox")) {
    setFieldError("coords", message);
    return;
  }
  if (lower.includes("vending_type") || lower.includes("種別")) {
    setFieldError("vending_type", message);
    return;
  }
  if (lower.includes("name") || lower.includes("名前")) {
    setFieldError("name", message);
    return;
  }
  if (lower.includes("operator") || lower.includes("運営")) {
    setFieldError("operator", message);
    return;
  }
  if (lower.includes("opening") || lower.includes("営業")) {
    setFieldError("opening_hours", message);
  }
}

function normalizeString(value) {
  if (typeof value !== "string") {
    return null;
  }
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}

async function safeJson(response) {
  try {
    return await response.json();
  } catch (_) {
    return null;
  }
}

function readErrorMessage(payload, fallback) {
  if (payload && payload.error && typeof payload.error.message === "string") {
    return payload.error.message;
  }
  return fallback;
}

function loadVotedMap() {
  try {
    const raw = window.localStorage.getItem(VOTED_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch (_) {
    return {};
  }
}

function saveVotedMap(votedMap) {
  window.localStorage.setItem(VOTED_KEY, JSON.stringify(votedMap));
}

function todayKey() {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, "0");
  const d = String(now.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

async function loadImagesForPopup(machineId, container) {
  const listEl = container.querySelector(".popup-images");
  if (!listEl) {
    return;
  }

  try {
    const res = await fetch(`${API_BASE}/vending-machines/${machineId}/images`);
    if (!res.ok) {
      listEl.textContent = "画像の取得に失敗しました";
      return;
    }
    const images = await res.json();
    listEl.replaceChildren();

    if (images.length === 0) {
      listEl.textContent = "画像なし";
      return;
    }

    images.forEach((img) => {
      const thumb = document.createElement("img");
      thumb.src = img.image_url;
      thumb.alt = "自販機画像";
      thumb.className = "popup-thumb";
      thumb.addEventListener("click", () => {
        const itemsList = container.querySelector(".popup-items-list");
        if (itemsList) {
          renderItemsList(img.items || [], itemsList);
        }
      });
      listEl.appendChild(thumb);
    });

    const firstItems = images[0]?.items || [];
    const itemsList = container.querySelector(".popup-items-list");
    if (itemsList) {
      renderItemsList(firstItems, itemsList);
    }
  } catch (_) {
    if (listEl) {
      listEl.textContent = "画像の取得に失敗しました";
    }
  }
}

async function uploadImage(machineId, file, container, uploadMessage) {
  const uploadBtn = container.querySelector(".popup-upload-btn");
  const itemsList = container.querySelector(".popup-items-list");

  if (uploadBtn) {
    uploadBtn.disabled = true;
    uploadBtn.textContent = "解析中...";
  }
  if (uploadMessage) {
    uploadMessage.textContent = "画像をアップロードしています...";
  }

  const formData = new FormData();
  formData.append("image", file);

  try {
    const res = await fetch(`${API_BASE}/vending-machines/${machineId}/images`, {
      method: "POST",
      body: formData,
    });
    const data = await safeJson(res);
    if (!res.ok) {
      throw new Error(readErrorMessage(data, "アップロードに失敗しました"));
    }

    const items = data?.image?.items || [];
    if (uploadBtn) {
      uploadBtn.textContent = items.length > 0
        ? `解析完了 (${items.length}件)`
        : "解析完了（商品なし）";
    }
    if (uploadMessage) {
      uploadMessage.textContent = items.length > 0
        ? "解析結果を表示しました。別画像を選ぶと再解析できます。"
        : "解析完了。商品情報は検出されませんでした。";
    }

    if (itemsList) {
      renderItemsList(items, itemsList);
    }

    await loadImagesForPopup(machineId, container);
  } catch (err) {
    if (uploadBtn) {
      uploadBtn.textContent = "エラー";
      uploadBtn.disabled = false;
    }
    if (uploadMessage) {
      uploadMessage.textContent = err.message;
    }
    updateStatus(err.message);
  }
}

function renderItemsList(items, listEl) {
  listEl.replaceChildren();
  if (!items || items.length === 0) {
    const li = document.createElement("li");
    li.textContent = "商品情報なし";
    listEl.appendChild(li);
    return;
  }
  const tempIcon = { hot: "🔴", cold: "🔵", warm: "🟡", unknown: "⚪" };
  items.forEach((item) => {
    const li = document.createElement("li");
    const icon = tempIcon[item.temperature] || "⚪";
    const price = item.price_yen != null ? ` ¥${item.price_yen}` : "";
    const name = item.name ? `${item.name} ` : "";
    li.textContent = `${icon} ${name}${price}`.trim();
    listEl.appendChild(li);
  });
}

function statusLabel(status) {
  if (status === "active") {
    return "稼働中";
  }
  if (status === "suspected_removed") {
    return "要確認";
  }
  if (status === "removed") {
    return "撤去";
  }
  return "未確認";
}
