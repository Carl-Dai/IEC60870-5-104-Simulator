export type DictShape = {
  common: { confirm: string; cancel: string; ok: string }
  toolbar: {
    newConnection: string
    connect: string
    disconnect: string
    delete: string
    sendGI: string
    clockSync: string
    counterRead: string
    appTitle: string
    about: string
  }
  _test: { interp: string }
}

const dict: DictShape = {
  common: { confirm: '确认', cancel: '取消', ok: '确定' },
  toolbar: {
    newConnection: '新建连接',
    connect: '连接',
    disconnect: '断开',
    delete: '删除',
    sendGI: '总召唤',
    clockSync: '时钟同步',
    counterRead: '累计量召唤',
    appTitle: 'IEC104 Master',
    about: '关于',
  },
  _test: {
    interp: '订单 #{id} 由 {user} 创建',
  },
}

export default dict
