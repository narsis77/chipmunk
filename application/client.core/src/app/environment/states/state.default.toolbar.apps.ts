import * as Toolkit from 'chipmunk.client.toolkit';

import { ViewSearchComponent } from '../components/views/search/component';
import { SidebarAppNotificationsComponent } from '../components/views/notifications/component';
import { SidebarAppNotificationsCounterComponent } from '../components/views/notifications/counter/component';
import { ViewChartComponent } from '../components/views/chart/component';
import { ViewTerminalComponent } from '../components/views/terminal/component';
import { ViewMeasurementComponent } from '../components/views/measurement/component';

export interface IDefaultTabsGuids {
    search: string;
    charts: string;
    notification: string;
    terminal: string;
    timemeasurement: string;
}

export const CDefaultTabsGuids: IDefaultTabsGuids = {
    search: Toolkit.guid(),
    charts: Toolkit.guid(),
    notification: Toolkit.guid(),
    terminal: Toolkit.guid(),
    timemeasurement: Toolkit.guid(),
};

export interface IDefaultView {
    name: string;
    guid: string;
    factory: any;
    tabCaptionInjection?: any;
    inputs: { [key: string]: any };
    default?: boolean;
}

export const DefaultViews: IDefaultView[] = [
    {
        name: 'Time Measurement',
        guid: CDefaultTabsGuids.timemeasurement,
        factory: ViewMeasurementComponent,
        inputs: { }
    },
    {
        name: 'Terminal',
        guid: CDefaultTabsGuids.terminal,
        factory: ViewTerminalComponent,
        inputs: { }
    },
    {
        name: 'Charts',
        guid: CDefaultTabsGuids.charts,
        factory: ViewChartComponent,
        inputs: { }
    },
    {
        name: 'Notifications',
        guid: CDefaultTabsGuids.notification,
        factory: SidebarAppNotificationsComponent,
        tabCaptionInjection: SidebarAppNotificationsCounterComponent,
        inputs: { }
    },
    {
        name: 'Search',
        guid: CDefaultTabsGuids.search,
        factory: ViewSearchComponent,
        inputs: { },
        default: true,
    },
];
