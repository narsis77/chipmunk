import { events as Events               } from '../modules/controller.events';
import { configuration as Configuration } from '../modules/controller.config';
import { WSCommandMessage               } from './ws.message.interface';
import { Logs, TYPES                    } from '../modules/tools.logs';

const COMMANDS = {
    greeting                : 'greeting',
    GUIDAccepted            : 'GUIDAccepted',
    SerialData              : 'SerialData',
    WriteToSerial           : 'WriteToSerial',
    ResultWrittenToSerial   : 'ResultWrittenToSerial',
    UpdateIsAvailable       : 'UpdateIsAvailable',
    UpdateDownloadProgress  : 'UpdateDownloadProgress',
    ADBLogcatData           : 'ADBLogcatData'
};

class WSCommands{
    private GUID: string = null;

    constructor(GUID: string){
        this.GUID = GUID;
    }

    proceed(message : WSCommandMessage, sender: Function){
        if (this[message.command] !== void 0){
            this[message.command](message, sender);
            return true;
        } else {
            Logs.msg(_('WebSocket server send unknown command: ') + message.command, TYPES.ERROR);
            return false;
        }
    }

    /* * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * *
     * Commands
     * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * * */
    //OUTGOING
    [COMMANDS.greeting                  ](message : WSCommandMessage, sender: Function){
        sender({
            GUID    : this.GUID,
            command : COMMANDS.greeting,
            params  : {}
        });
    }

    [COMMANDS.WriteToSerial             ](message : WSCommandMessage, sender: Function){
        sender({
            GUID    : this.GUID,
            command : COMMANDS.WriteToSerial,
            params  : message.params
        });
    }

    //INCOME
    [COMMANDS.GUIDAccepted              ](message : WSCommandMessage, sender: Function){
        if (this.GUID === message.GUID){
            Events.trigger(Configuration.sets.SYSTEM_EVENTS.API_GUID_IS_ACCEPTED, message.GUID);
            Logs.msg(_('Client GUID was accepted by server. GUID: ') + this.GUID, TYPES.DEBUG);
        } else {
            Logs.msg(_('Incorrect GUID was gotten from server. Original / from server: ') + this.GUID + ' / ' + message.GUID, TYPES.ERROR);
        }
    }

    [COMMANDS.SerialData                ](message : WSCommandMessage, sender: Function){
        if (typeof message.params === 'object' && message.params !== null && typeof message.params.connection === 'string' && typeof message.params.data === 'string'){
            Events.trigger(Configuration.sets.SYSTEM_EVENTS.SERIAL_DATA_COME, message.params);
        }
    }

    [COMMANDS.ResultWrittenToSerial     ](message : WSCommandMessage, sender: Function){
        if (typeof message.params === 'object' && message.params !== null && typeof message.params.serialGUID === 'string' && typeof message.params.packageGUID === 'string'){
            Events.trigger(Configuration.sets.SYSTEM_EVENTS.DATA_TO_SERIAL_SENT, message.params);
        }
    }

    [COMMANDS.UpdateIsAvailable         ](message : WSCommandMessage, sender: Function){
        if (typeof message.params === 'object' && message.params !== null){
            Events.trigger(Configuration.sets.SYSTEM_EVENTS.UPDATE_IS_AVAILABLE, message.params);
        }
    }

    [COMMANDS.UpdateDownloadProgress    ](message : WSCommandMessage, sender: Function){
        if (typeof message.params === 'object' && message.params !== null){
            Events.trigger(Configuration.sets.SYSTEM_EVENTS.UPDATE_DOWNLOAD_PROGRESS, message.params);
        }
    }

    [COMMANDS.ADBLogcatData             ](message : WSCommandMessage, sender: Function){
        if (typeof message.params === 'object' && message.params !== null && typeof message.params.stream === 'string' && message.params.entries instanceof Array){
            Events.trigger(Configuration.sets.SYSTEM_EVENTS.ADB_LOGCAT_DATA_COME, message.params);
        }
    }

}

export { WSCommands, COMMANDS };
