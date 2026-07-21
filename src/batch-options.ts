export type DetectionPlatform = "douyin" | "toutiao";

export type DouyinDetectionOptions = {
  token: boolean;
  password: boolean;
  certification: boolean;
  aid: boolean;
  registrationTime: boolean;
};

export type ToutiaoDetectionOptions = {
  token: boolean;
  certification: boolean;
};

export type BatchDetectionOptions = DouyinDetectionOptions & {
  appType: DetectionPlatform;
};

export function buildBatchDetectionOptions(
  platform: DetectionPlatform,
  douyin: DouyinDetectionOptions,
  toutiao: ToutiaoDetectionOptions,
): BatchDetectionOptions {
  if (platform === "douyin") {
    return { appType: platform, ...douyin };
  }

  return {
    appType: platform,
    token: toutiao.token,
    password: false,
    certification: toutiao.certification,
    aid: false,
    registrationTime: false,
  };
}

type StateUpdater<State> = (current: State) => State;
type StateDispatch<State> = (updater: StateUpdater<State>) => void;

export function queueBatchOptionFromEvent<
  State,
  Key extends keyof State,
  Target,
>(
  dispatch: StateDispatch<State>,
  key: Key,
  event: { currentTarget: Target },
  readValue: (target: Target) => State[Key],
) {
  const value = readValue(event.currentTarget);
  dispatch((current) => ({ ...current, [key]: value }));
}
